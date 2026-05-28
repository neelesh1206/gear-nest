"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { chatSseUrl, IS_USING_MOCKS } from "@/lib/api/client";
import { cn } from "@/lib/utils";

const SESSION_BUDGET = 5;

interface ChatMessage {
  role: "user" | "assistant";
  text: string;
  status: "streaming" | "done" | "error" | "limit";
}

export function ChatPanel({
  productId,
  productName,
}: {
  productId: string;
  productName: string;
}) {
  const [query, setQuery] = useState("");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [remaining, setRemaining] = useState<number>(SESSION_BUDGET);
  const [streaming, setStreaming] = useState(false);
  const esRef = useRef<EventSource | null>(null);
  const mockTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    return () => {
      esRef.current?.close();
      if (mockTimerRef.current) clearInterval(mockTimerRef.current);
    };
  }, []);

  const appendAssistantToken = useCallback((token: string) => {
    setMessages((prev) => {
      const last = prev[prev.length - 1];
      if (!last || last.role !== "assistant" || last.status !== "streaming") return prev;
      const updated = [...prev];
      updated[updated.length - 1] = { ...last, text: last.text + token };
      return updated;
    });
  }, []);

  const finalizeAssistant = useCallback((status: ChatMessage["status"]) => {
    setMessages((prev) => {
      const last = prev[prev.length - 1];
      if (!last || last.role !== "assistant") return prev;
      const updated = [...prev];
      updated[updated.length - 1] = { ...last, status };
      return updated;
    });
  }, []);

  const startMockStream = useCallback(
    (userQuery: string) => {
      const tokens =
        `Based on indexed reviews and specs for ${productName}, here's what stands out. ` +
        `Multiple owners highlight the build quality holds up across multi-day trips. ` +
        `One critical pattern: ${userQuery.toLowerCase().includes("rain") ? "wet conditions" : "extreme cold"} performance is mixed — a small set of users reported degraded behavior outside the stated rating. ` +
        `Net: a confident yes for typical conditions; verify edge-case ratings before relying on it in worst-case weather.`;
      const parts = tokens.split(/(\s+)/);
      let i = 0;
      mockTimerRef.current = setInterval(() => {
        if (i >= parts.length) {
          if (mockTimerRef.current) clearInterval(mockTimerRef.current);
          mockTimerRef.current = null;
          setRemaining((r) => Math.max(0, r - 1));
          finalizeAssistant("done");
          setStreaming(false);
          return;
        }
        appendAssistantToken(parts[i] ?? "");
        i += 1;
      }, 30);
    },
    [appendAssistantToken, finalizeAssistant, productName],
  );

  const startSseStream = useCallback(
    (userQuery: string) => {
      const url = chatSseUrl(userQuery, productId);
      const es = new EventSource(url, { withCredentials: true });
      esRef.current = es;

      es.addEventListener("token", (evt) => {
        const msg = evt as MessageEvent<string>;
        appendAssistantToken(msg.data);
      });
      es.addEventListener("done", (evt) => {
        const msg = evt as MessageEvent<string>;
        try {
          const payload = JSON.parse(msg.data) as { remaining: number };
          if (typeof payload.remaining === "number") setRemaining(payload.remaining);
        } catch {
          // payload not JSON — leave budget untouched
        }
        finalizeAssistant("done");
        setStreaming(false);
        es.close();
      });
      es.addEventListener("limit_reached", () => {
        finalizeAssistant("limit");
        setRemaining(0);
        setStreaming(false);
        es.close();
      });
      es.addEventListener("error", () => {
        finalizeAssistant("error");
        setStreaming(false);
        es.close();
      });
    },
    [appendAssistantToken, finalizeAssistant, productId],
  );

  const submit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const trimmed = query.trim();
    if (!trimmed || streaming || remaining <= 0) return;
    setMessages((prev) => [
      ...prev,
      { role: "user", text: trimmed, status: "done" },
      { role: "assistant", text: "", status: "streaming" },
    ]);
    setQuery("");
    setStreaming(true);
    if (IS_USING_MOCKS) startMockStream(trimmed);
    else startSseStream(trimmed);
  };

  const exhausted = remaining <= 0;

  return (
    <section className="rounded-lg border border-border bg-card text-card-foreground flex flex-col h-[28rem]">
      <header className="flex items-center justify-between border-b border-border px-4 py-3">
        <div>
          <h2 className="text-sm font-semibold">Ask about this product</h2>
          <p className="text-xs text-muted-foreground">Answers ground in indexed reviews and specs.</p>
        </div>
        <BudgetIndicator remaining={remaining} total={SESSION_BUDGET} />
      </header>

      <div className="flex-1 overflow-y-auto px-4 py-3 space-y-3 text-sm">
        {messages.length === 0 ? (
          <p className="text-muted-foreground">
            Try: <em>&ldquo;Is this good for 4-season alpine use?&rdquo;</em>
          </p>
        ) : null}
        {messages.map((m, i) => (
          <Message key={i} message={m} />
        ))}
      </div>

      <form onSubmit={submit} className="border-t border-border p-3 flex gap-2">
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={exhausted ? "Session limit reached" : "Ask a question…"}
          disabled={streaming || exhausted}
          className={cn(
            "flex-1 rounded-md border border-input bg-background px-3 py-2 text-sm",
            "focus:outline-none focus:ring-2 focus:ring-brand/40 disabled:opacity-60",
          )}
        />
        <button
          type="submit"
          disabled={streaming || exhausted || query.trim() === ""}
          className={cn(
            "rounded-md bg-primary text-primary-foreground px-3 py-2 text-sm font-medium",
            "disabled:opacity-50 disabled:cursor-not-allowed",
          )}
        >
          {streaming ? "…" : "Ask"}
        </button>
      </form>
    </section>
  );
}

function Message({ message }: { message: ChatMessage }) {
  const isUser = message.role === "user";
  return (
    <div className={cn("flex", isUser ? "justify-end" : "justify-start")}>
      <div
        className={cn(
          "max-w-[85%] rounded-lg px-3 py-2",
          isUser ? "bg-primary text-primary-foreground" : "bg-muted text-foreground",
        )}
      >
        <p className="whitespace-pre-wrap leading-relaxed">
          {message.text}
          {message.status === "streaming" ? (
            <span className="inline-block w-2 h-4 align-text-bottom bg-current animate-pulse ml-0.5" />
          ) : null}
        </p>
        {message.status === "limit" ? (
          <p className="mt-1 text-[11px] text-muted-foreground">
            Session limit reached. Resets in 2 hours.
          </p>
        ) : null}
        {message.status === "error" ? (
          <p className="mt-1 text-[11px] text-danger">
            Chat failed. Budget restored — try again.
          </p>
        ) : null}
      </div>
    </div>
  );
}

function BudgetIndicator({ remaining, total }: { remaining: number; total: number }) {
  return (
    <div
      className="flex items-center gap-1.5 text-xs text-muted-foreground"
      title={`${remaining} of ${total} questions remaining this 2-hour session`}
    >
      {Array.from({ length: total }).map((_, i) => (
        <span
          key={i}
          className={cn(
            "h-1.5 w-3 rounded-full",
            i < remaining ? "bg-brand" : "bg-border",
          )}
          aria-hidden
        />
      ))}
      <span>{remaining}/{total}</span>
    </div>
  );
}
