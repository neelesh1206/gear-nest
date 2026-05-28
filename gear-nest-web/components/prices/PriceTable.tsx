import type { StoreListing } from "@/lib/api/types";
import { cn, formatCurrency, formatRelative } from "@/lib/utils";

export function PriceTable({
  listings,
  lastUpdated,
  nextUpdate,
}: {
  listings: StoreListing[];
  lastUpdated?: string | null;
  nextUpdate?: string | null;
}) {
  const visible = listings
    .filter((l) => l.matchConfidence !== "CANDIDATE")
    .sort((a, b) => (b.bestValueScore ?? 0) - (a.bestValueScore ?? 0));

  if (visible.length === 0) {
    return (
      <div className="rounded-md border border-dashed border-border p-6 text-sm text-muted-foreground">
        No confirmed listings yet.
      </div>
    );
  }

  return (
    <section className="rounded-lg border border-border bg-card text-card-foreground">
      <header className="flex items-center justify-between px-4 py-3 border-b border-border">
        <div>
          <h2 className="text-sm font-semibold">Price comparison</h2>
          <p className="text-xs text-muted-foreground">
            Updated {formatRelative(lastUpdated)} · refresh {formatRelative(nextUpdate)}
          </p>
        </div>
        <span className="text-xs text-muted-foreground">{visible.length} stores</span>
      </header>
      <ul role="list" className="divide-y divide-border">
        {visible.map((l) => (
          <PriceRow key={l.id} listing={l} />
        ))}
      </ul>
    </section>
  );
}

function PriceRow({ listing }: { listing: StoreListing }) {
  const confidenceLabel =
    listing.matchConfidence === "EXACT" || listing.matchConfidence === "HIGH"
      ? null
      : listing.matchConfidence;

  return (
    <li
      className={cn(
        "flex flex-wrap items-center gap-3 px-4 py-3",
        listing.isBestValue && "bg-brand/5",
      )}
    >
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="font-medium">{listing.store.displayName}</span>
          {listing.isBestValue ? (
            <span className="rounded-full bg-brand text-brand-foreground text-[10px] font-semibold uppercase px-2 py-0.5">
              Best value
            </span>
          ) : null}
          {listing.isStale ? (
            <span
              className="rounded-full bg-warning/15 text-warning text-[10px] font-semibold uppercase px-2 py-0.5"
              title="Price last fetched over 24 hours ago"
            >
              Stale
            </span>
          ) : null}
          {confidenceLabel ? (
            <span className="rounded-full bg-muted text-muted-foreground text-[10px] uppercase px-2 py-0.5">
              {confidenceLabel}
            </span>
          ) : null}
          {listing.inStock === false ? (
            <span className="rounded-full bg-muted text-muted-foreground text-[10px] uppercase px-2 py-0.5">
              Out of stock
            </span>
          ) : null}
        </div>
        <div className="text-xs text-muted-foreground mt-0.5">
          {listing.storeRating != null ? `★ ${listing.storeRating.toFixed(1)}` : "No rating"}
          {listing.reviewCount > 0 ? ` · ${listing.reviewCount.toLocaleString()} reviews` : ""}
          {" · "}
          {listing.priceFetchedAt ? `seen ${formatRelative(listing.priceFetchedAt)}` : "freshness unknown"}
        </div>
      </div>
      <div className="text-right min-w-[7rem]">
        <div className={cn("text-base font-semibold", listing.isBestValue && "text-brand")}>
          {formatCurrency(listing.price, listing.currency)}
        </div>
        {listing.bestValueScore != null ? (
          <div className="text-[10px] text-muted-foreground">
            value {(listing.bestValueScore * 100).toFixed(0)}
          </div>
        ) : null}
      </div>
      <a
        href={listing.affiliateUrl ?? listing.storeUrl}
        target="_blank"
        rel="noreferrer"
        className={cn(
          "inline-flex items-center rounded-md px-3 py-1.5 text-sm font-medium border border-border",
          "hover:bg-muted transition",
        )}
      >
        Visit
      </a>
    </li>
  );
}
