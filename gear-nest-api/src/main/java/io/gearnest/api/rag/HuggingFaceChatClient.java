package io.gearnest.api.rag;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import io.gearnest.api.config.HuggingFaceProperties;
import org.springframework.http.MediaType;
import org.springframework.stereotype.Component;
import org.springframework.web.reactive.function.client.WebClient;
import reactor.core.publisher.Flux;

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
        return webClient.post()
            .uri("/models/{model}", props.chatModel())
            .contentType(MediaType.APPLICATION_JSON)
            .accept(MediaType.TEXT_EVENT_STREAM)
            .bodyValue(Map.of(
                "inputs", prompt,
                "parameters", Map.of("max_new_tokens", 512, "temperature", 0.3, "return_full_text", false),
                "stream", true,
                "options", Map.of("wait_for_model", true)
            ))
            .retrieve()
            .bodyToFlux(String.class)
            .mapNotNull(this::extractToken)
            .filter(t -> !t.isEmpty());
    }

    private String extractToken(String chunk) {
        try {
            JsonNode node = json.readTree(chunk);
            JsonNode token = node.path("token").path("text");
            if (!token.isMissingNode()) return token.asText();
            JsonNode generated = node.path("generated_text");
            if (!generated.isMissingNode()) return generated.asText();
        } catch (Exception ignored) {}
        return "";
    }
}
