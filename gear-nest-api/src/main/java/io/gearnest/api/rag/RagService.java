package io.gearnest.api.rag;

import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.ObjectMapper;
import io.gearnest.api.config.RagProperties;
import io.gearnest.api.embedding.EmbeddingClient;
import io.gearnest.api.product.ProductRepository;
import io.gearnest.api.session.SessionBudgetService;
import org.springframework.stereotype.Service;
import org.springframework.web.servlet.mvc.method.annotation.SseEmitter;

import java.io.IOException;
import java.time.Duration;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;

@Service
public class RagService {

    private final EmbeddingClient embeddings;
    private final ChatClient chatClient;
    private final ChunkRepository chunks;
    private final MmrSelector mmr;
    private final PromptBuilder prompts;
    private final SessionBudgetService sessions;
    private final ProductRepository products;
    private final RagProperties cfg;
    private final ObjectMapper json;

    public RagService(EmbeddingClient embeddings, ChatClient chatClient, ChunkRepository chunks,
                      MmrSelector mmr, PromptBuilder prompts, SessionBudgetService sessions,
                      ProductRepository products, RagProperties cfg, ObjectMapper json) {
        this.embeddings = embeddings;
        this.chatClient = chatClient;
        this.chunks = chunks;
        this.mmr = mmr;
        this.prompts = prompts;
        this.sessions = sessions;
        this.products = products;
        this.cfg = cfg;
        this.json = json;
    }

    public void streamAnswer(String query, UUID productId, String sessionId, SseEmitter emitter) {
        SessionBudgetService.ReserveResult reserved = sessions.reserve(sessionId);
        if (!reserved.reserved()) {
            sendEvent(emitter, "limit_reached", "{}");
            emitter.complete();
            return;
        }

        String productName = products.findNameById(productId).orElse("the product");
        float[] queryEmbedding = embeddings.embed(query);

        List<Chunk> overfetch = chunks.findReviewChunksByCosine(productId, queryEmbedding, cfg.overfetch());
        List<Chunk> semantic = mmr.select(overfetch, queryEmbedding, cfg.semanticK(), cfg.mmrLambda());
        List<Chunk> negative = chunks.findNegativeReviews(productId, queryEmbedding, cfg.negativeK());
        List<Chunk> specs = chunks.findSpecChunks(productId, queryEmbedding, cfg.specK());

        String prompt = prompts.build(productName, query, semantic, negative, specs);

        AtomicBoolean committed = new AtomicBoolean(false);
        AtomicReference<Throwable> failure = new AtomicReference<>();

        try {
            chatClient.stream(prompt)
                .timeout(Duration.ofSeconds(cfg.firstTokenTimeoutSeconds()))
                .doOnNext(token -> {
                    if (committed.compareAndSet(false, true)) {
                        sessions.commit(sessionId);
                    }
                    sendEvent(emitter, "token", token);
                })
                .doOnError(err -> {
                    failure.set(err);
                    if (!committed.get()) {
                        sessions.rollback(sessionId);
                        sendEvent(emitter, "error", asJson(Map.of("budgetRestored", true)));
                    } else {
                        sendEvent(emitter, "error", asJson(Map.of("budgetRestored", false)));
                    }
                })
                .onErrorResume(e -> reactor.core.publisher.Flux.empty())
                .blockLast();
        } catch (Exception e) {
            failure.set(e);
            if (!committed.get()) {
                sessions.rollback(sessionId);
                sendEvent(emitter, "error", asJson(Map.of("budgetRestored", true)));
            }
        }

        if (failure.get() == null) {
            sendEvent(emitter, "done", asJson(Map.of(
                "remaining", sessions.remaining(sessionId),
                "sessionId", sessionId
            )));
        }
        emitter.complete();
    }

    private void sendEvent(SseEmitter emitter, String name, String data) {
        try {
            emitter.send(SseEmitter.event().name(name).data(data));
        } catch (IOException ignored) {}
    }

    private String asJson(Map<String, ?> data) {
        try {
            return json.writeValueAsString(data);
        } catch (JsonProcessingException e) {
            return "{}";
        }
    }
}
