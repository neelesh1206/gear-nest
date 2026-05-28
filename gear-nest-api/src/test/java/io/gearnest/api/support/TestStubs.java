package io.gearnest.api.support;

import io.gearnest.api.embedding.EmbeddingClient;
import io.gearnest.api.rag.ChatClient;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Primary;
import reactor.core.publisher.Flux;

import java.time.Duration;

@TestConfiguration
public class TestStubs {

    public static final String SEEDED_PRODUCT_NAME = "MSR Reactor Stove";

    @Bean
    @Primary
    public EmbeddingClient stubEmbeddingClient() {
        return text -> {
            float[] v = new float[384];
            int seed = text == null ? 0 : text.hashCode();
            for (int i = 0; i < v.length; i++) {
                v[i] = (float) Math.sin(seed + i) * 0.1f;
            }
            return v;
        };
    }

    @Bean
    @Primary
    public ChatClient stubChatClient() {
        return prompt -> Flux.just(
            "The ",
            SEEDED_PRODUCT_NAME + " ",
            "performs well in cold weather."
        ).delayElements(Duration.ofMillis(10));
    }
}
