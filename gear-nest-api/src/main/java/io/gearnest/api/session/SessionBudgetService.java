package io.gearnest.api.session;

import io.gearnest.api.config.SessionProperties;
import org.springframework.dao.DataAccessException;
import org.springframework.data.redis.connection.RedisConnection;
import org.springframework.data.redis.core.RedisOperations;
import org.springframework.data.redis.core.SessionCallback;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.stereotype.Service;

import java.time.Duration;
import java.util.List;
import java.util.Optional;
import java.util.UUID;

@Service
public class SessionBudgetService {

    private final StringRedisTemplate redis;
    private final SessionProperties props;

    public SessionBudgetService(StringRedisTemplate redis, SessionProperties props) {
        this.redis = redis;
        this.props = props;
    }

    public String ensureSession(String existing) {
        if (existing != null && !existing.isBlank()) return existing;
        String id = UUID.randomUUID().toString();
        redis.opsForValue().set(remainingKey(id), Integer.toString(props.questionLimit()),
            Duration.ofHours(props.ttlHours()));
        return id;
    }

    public int remaining(String sessionId) {
        if (sessionId == null) return props.questionLimit();
        String v = redis.opsForValue().get(remainingKey(sessionId));
        if (v == null) {
            redis.opsForValue().set(remainingKey(sessionId), Integer.toString(props.questionLimit()),
                Duration.ofHours(props.ttlHours()));
            return props.questionLimit();
        }
        return Integer.parseInt(v);
    }

    public ReserveResult reserve(String sessionId) {
        String remainingKey = remainingKey(sessionId);
        String inflightKey = inflightKey(sessionId);

        @SuppressWarnings({"unchecked", "rawtypes"})
        List<Object> tx = redis.execute(new SessionCallback<List<Object>>() {
            @Override
            public List<Object> execute(RedisOperations operations) throws DataAccessException {
                operations.watch(remainingKey);
                String current = (String) operations.opsForValue().get(remainingKey);
                int remaining;
                if (current == null) {
                    remaining = props.questionLimit();
                    operations.opsForValue().set(remainingKey, Integer.toString(remaining),
                        Duration.ofHours(props.ttlHours()));
                    operations.watch(remainingKey);
                    current = Integer.toString(remaining);
                } else {
                    remaining = Integer.parseInt(current);
                }
                if (remaining <= 0) {
                    operations.unwatch();
                    return null;
                }
                operations.multi();
                operations.opsForValue().decrement(remainingKey);
                operations.opsForValue().set(inflightKey, "1",
                    Duration.ofSeconds(props.inflightTtlSeconds()));
                return operations.exec();
            }
        });
        if (tx == null || tx.isEmpty()) return ReserveResult.LIMIT_REACHED;

        return new ReserveResult(true, false, remaining(sessionId));
    }

    public void commit(String sessionId) {
        redis.delete(inflightKey(sessionId));
    }

    public void rollback(String sessionId) {
        redis.opsForValue().increment(remainingKey(sessionId));
        redis.delete(inflightKey(sessionId));
    }

    public Optional<Boolean> isInflight(String sessionId) {
        return Optional.ofNullable(redis.hasKey(inflightKey(sessionId)));
    }

    private String remainingKey(String sid) { return "session:" + sid + ":questions_remaining"; }
    private String inflightKey(String sid)  { return "session:" + sid + ":inflight"; }

    public record ReserveResult(boolean reserved, boolean limitReached, int remaining) {
        static final ReserveResult LIMIT_REACHED = new ReserveResult(false, true, 0);
    }
}
