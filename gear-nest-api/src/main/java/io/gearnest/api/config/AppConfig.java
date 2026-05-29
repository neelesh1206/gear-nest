package io.gearnest.api.config;

import org.springframework.boot.context.properties.EnableConfigurationProperties;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.scheduling.annotation.EnableAsync;
import org.springframework.web.reactive.function.client.WebClient;

@Configuration
@EnableAsync
@EnableConfigurationProperties({HuggingFaceProperties.class, SessionProperties.class, RagProperties.class})
public class AppConfig {

    @Bean
    public WebClient huggingFaceWebClient(HuggingFaceProperties props) {
        WebClient.Builder builder = WebClient.builder().baseUrl(props.baseUrl());
        if (props.apiKey() != null && !props.apiKey().isBlank()) {
            builder.defaultHeader("Authorization", "Bearer " + props.apiKey());
        }
        return builder.build();
    }
}
