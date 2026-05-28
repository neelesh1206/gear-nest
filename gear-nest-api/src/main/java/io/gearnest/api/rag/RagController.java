package io.gearnest.api.rag;

import io.gearnest.api.session.SessionBudgetService;
import jakarta.servlet.http.Cookie;
import jakarta.servlet.http.HttpServletResponse;
import org.springframework.http.MediaType;
import org.springframework.web.bind.annotation.CookieValue;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;
import org.springframework.web.servlet.mvc.method.annotation.SseEmitter;

import java.util.UUID;

@RestController
@RequestMapping("/api/v1")
public class RagController {

    private static final String COOKIE_NAME = "gn_session";
    private static final long EMITTER_TIMEOUT_MS = 60_000L;

    private final RagService rag;
    private final SessionBudgetService sessions;

    public RagController(RagService rag, SessionBudgetService sessions) {
        this.rag = rag;
        this.sessions = sessions;
    }

    @GetMapping(value = "/chat", produces = MediaType.TEXT_EVENT_STREAM_VALUE)
    public SseEmitter chat(
        @RequestParam String query,
        @RequestParam UUID productId,
        @CookieValue(name = COOKIE_NAME, required = false) String sessionCookie,
        HttpServletResponse response
    ) {
        String sessionId = sessions.ensureSession(sessionCookie);
        if (sessionCookie == null || sessionCookie.isBlank()) {
            Cookie c = new Cookie(COOKIE_NAME, sessionId);
            c.setHttpOnly(true);
            c.setPath("/");
            c.setMaxAge(2 * 60 * 60);
            response.addCookie(c);
        }

        SseEmitter emitter = new SseEmitter(EMITTER_TIMEOUT_MS);
        Thread.ofVirtual().start(() -> {
            try {
                rag.streamAnswer(query, productId, sessionId, emitter);
            } catch (Exception e) {
                emitter.completeWithError(e);
            }
        });
        return emitter;
    }
}
