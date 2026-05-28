package io.gearnest.api.rag;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import io.gearnest.api.config.HuggingFaceProperties;
import org.springframework.http.MediaType;
import org.springframework.stereotype.Component;
import org.springframework.web.reactive.function.client.WebClient;
import reactor.core.publisher.Flux;

import java.util.List;
import java.util.Map;

@Component
public class HuggingFaceChatClient implements ChatClient {

    private final WebClient webClient;
    private final HuggingFaceProperties props;
    private final ObjectMapper json;

    public HuggingFaceChatClient(WebClient huggingFaceWebClient, HuggingFaceProperties props, ObjectMapper json) {
        this.webClient = huggingFaceWebClient;
        this.props = props;
        this.json = json;
    }

    @Override
    public Flux<String> stream(String prompt) {
        if (props.apiKey() == null || props.apiKey().isBlank()) {
            return Flux.error(new IllegalStateException("HuggingFace API key not configured"));
        }
        // HF router is OpenAI-compatible: POST /v1/chat/completions with
        // {model, messages, stream:true} → SSE chunks of {choices:[{delta:{content}}]}.
        return webClient.post()
            .uri("/v1/chat/completions")
            .contentType(MediaType.APPLICATION_JSON)
            .accept(MediaType.TEXT_EVENT_STREAM)
            .bodyValue(Map.of(
                "model", props.chatModel(),
                "messages", List.of(Map.of("role", "user", "content", prompt)),
                "max_tokens", 512,
                "temperature", 0.3,
                "stream", true
            ))
            .retrieve()
            .bodyToFlux(String.class)
            .takeUntil("[DONE]"::equals)
            .mapNotNull(this::extractToken)
            .filter(t -> !t.isEmpty());
    }

    private String extractToken(String chunk) {
        if (chunk == null || chunk.isBlank() || "[DONE]".equals(chunk.trim())) return "";
        try {
            JsonNode content = json.readTree(chunk).path("choices").path(0).path("delta").path("content");
            if (content.isTextual()) return content.asText();
        } catch (Exception ignored) {}
        return "";
    }
}
