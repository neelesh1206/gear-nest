package io.gearnest.api.rag;

import reactor.core.publisher.Flux;

public interface ChatClient {
    Flux<String> stream(String prompt);
}
