import type { ReviewBreakdown } from "@/lib/api/types";
import { cn } from "@/lib/utils";

const TIERS: (keyof ReviewBreakdown["tiers"])[] = ["5", "4", "3", "2", "1"];

export function ReviewTierList({ breakdown }: { breakdown: ReviewBreakdown }) {
  return (
    <section className="space-y-6">
      <header className="flex items-baseline justify-between">
        <h2 className="text-lg font-semibold">Reviews</h2>
        <p className="text-xs text-muted-foreground">
          {breakdown.total.toLocaleString()} reviews aggregated across stores
        </p>
      </header>
      <div className="space-y-4">
        {TIERS.map((tier) => {
          const t = breakdown.tiers[tier];
          if (!t || t.count === 0) return null;
          const pct = breakdown.total === 0 ? 0 : Math.round((t.count / breakdown.total) * 100);
          return (
            <article
              key={tier}
              className="rounded-lg border border-border bg-card text-card-foreground p-4 space-y-3"
            >
              <header className="flex items-center justify-between">
                <h3 className="text-sm font-semibold flex items-center gap-2">
                  <span className="inline-flex items-center gap-0.5 text-amber-500" aria-hidden>
                    {Array.from({ length: Number(tier) }).map((_, i) => (
                      <span key={i}>★</span>
                    ))}
                    {Array.from({ length: 5 - Number(tier) }).map((_, i) => (
                      <span key={i} className="text-muted-foreground">☆</span>
                    ))}
                  </span>
                  <span className="text-muted-foreground text-xs">
                    {t.count.toLocaleString()} · {pct}%
                  </span>
                </h3>
              </header>
              <ul className="space-y-3">
                {t.sample.map((r) => (
                  <li
                    key={r.id}
                    className={cn(
                      "rounded-md border border-border/60 p-3 text-sm",
                      Number(tier) <= 2 ? "bg-warning/5" : "bg-background",
                    )}
                  >
                    <p className="leading-relaxed">&ldquo;{r.body}&rdquo;</p>
                    <footer className="mt-2 text-xs text-muted-foreground flex items-center gap-2">
                      <span>{r.store.displayName}</span>
                      {r.verifiedPurchase ? <span className="text-success">· verified purchase</span> : null}
                      {r.reviewDate ? <span>· {r.reviewDate}</span> : null}
                    </footer>
                  </li>
                ))}
              </ul>
            </article>
          );
        })}
      </div>
    </section>
  );
}
