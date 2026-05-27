package io.gearnest.api.rag;

import io.gearnest.api.embedding.Vectors;
import org.springframework.stereotype.Component;

import java.util.ArrayList;
import java.util.List;

@Component
public class MmrSelector {

    public List<Chunk> select(List<Chunk> candidates, float[] queryEmbedding, int k, float lambda) {
        List<Chunk> remaining = new ArrayList<>(candidates);
        List<Chunk> chosen = new ArrayList<>(k);
        while (chosen.size() < k && !remaining.isEmpty()) {
            Chunk best = null;
            float bestScore = -Float.MAX_VALUE;
            for (Chunk c : remaining) {
                float relevance = Vectors.cosine(c.embedding(), queryEmbedding);
                float diversityPenalty = 0f;
                for (Chunk picked : chosen) {
                    diversityPenalty = Math.max(diversityPenalty, Vectors.cosine(c.embedding(), picked.embedding()));
                }
                float score = lambda * relevance - (1 - lambda) * diversityPenalty;
                if (score > bestScore) {
                    bestScore = score;
                    best = c;
                }
            }
            if (best == null) break;
            chosen.add(best);
            remaining.remove(best);
        }
        return chosen;
    }
}
