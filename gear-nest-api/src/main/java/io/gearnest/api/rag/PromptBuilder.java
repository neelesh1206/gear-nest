package io.gearnest.api.rag;

import org.springframework.stereotype.Component;

import java.util.List;

@Component
public class PromptBuilder {

    private static final String SYSTEM = """
        You are GearNest AI, an expert outdoor gear advisor.
        Answer only from the provided context.
        If the context does not contain enough information, say so.
        Be concise — 3-5 sentences max.""";

    public String build(String productName, String query, List<Chunk> semantic, List<Chunk> negative, List<Chunk> specs) {
        StringBuilder ctx = new StringBuilder();
        ctx.append("Product: ").append(productName).append("\n\n");
        appendBlock(ctx, "SPEC", specs);
        appendBlock(ctx, "POSITIVE_REVIEW", semantic);
        appendBlock(ctx, "CRITICAL_REVIEW", negative);
        return "<<SYS>>\n" + SYSTEM + "\n<</SYS>>\n\nContext:\n" + ctx + "\nUser: " + query + "\nAssistant: ";
    }

    private static void appendBlock(StringBuilder sb, String label, List<Chunk> chunks) {
        for (Chunk c : chunks) {
            sb.append('[').append(label).append("] ").append(c.text()).append('\n');
        }
    }
}
