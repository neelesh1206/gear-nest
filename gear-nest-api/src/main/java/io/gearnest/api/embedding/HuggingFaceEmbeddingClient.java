package io.gearnest.api.embedding;

import io.gearnest.api.config.HuggingFaceProperties;
import org.springframework.core.ParameterizedTypeReference;
import org.springframework.stereotype.Component;
import org.springframework.web.reactive.function.client.WebClient;

import java.time.Duration;
import java.util.List;
import java.util.Map;

@Component
public class HuggingFaceEmbeddingClient implements EmbeddingClient {

    private static final int DIM = 384;

    private final WebClient webClient;
    private final HuggingFaceProperties props;

    public HuggingFaceEmbeddingClient(WebClient huggingFaceWebClient, HuggingFaceProperties props) {
        this.webClient = huggingFaceWebClient;
        this.props = props;
    }

    @Override
    public float[] embed(String text) {
        if (props.apiKey() == null || props.apiKey().isBlank()) {
            return zeros();
        }
        try {
            // Model id contains a '/', so build the path by concatenation rather
            // than a URI template variable (which would percent-encode the slash).
            List<Float> embedding = webClient.post()
                .uri("/hf-inference/models/" + props.embeddingModel() + "/pipeline/feature-extraction")
                .bodyValue(Map.of("inputs", text))
                .retrieve()
                .bodyToMono(new ParameterizedTypeReference<List<Float>>() {})
                .timeout(Duration.ofSeconds(15))
                .block();
            if (embedding == null || embedding.size() != DIM) {
                return zeros();
            }
            float[] result = new float[DIM];
            for (int i = 0; i < DIM; i++) result[i] = embedding.get(i);
            return result;
        } catch (Exception e) {
            return zeros();
        }
    }

    private static float[] zeros() {
        return new float[DIM];
    }
}
