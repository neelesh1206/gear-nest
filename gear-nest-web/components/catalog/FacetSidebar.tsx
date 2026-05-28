import Link from "next/link";
import type { Facets, SortKey } from "@/lib/api/types";
import { cn } from "@/lib/utils";

type SearchParams = Record<string, string | string[] | undefined>;

function toQuery(params: SearchParams, overrides: Record<string, string | undefined>): string {
  const sp = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (typeof v === "string" && v !== "") sp.set(k, v);
  }
  for (const [k, v] of Object.entries(overrides)) {
    if (v == null || v === "") sp.delete(k);
    else sp.set(k, v);
  }
  const s = sp.toString();
  return s ? `/?${s}` : "/";
}

const PRICE_RANGES: { value: string; label: string; min?: number; max?: number }[] = [
  { value: "0-50", label: "Under $50", max: 50 },
  { value: "50-150", label: "$50 – $150", min: 50, max: 150 },
  { value: "150-400", label: "$150 – $400", min: 150, max: 400 },
  { value: "400+", label: "$400+", min: 400 },
];

const SORT_OPTIONS: { value: SortKey; label: string }[] = [
  { value: "relevance", label: "Relevance" },
  { value: "best_value", label: "Best value" },
  { value: "price_asc", label: "Price ↑" },
  { value: "rating_desc", label: "Rating ↓" },
];

export function FacetSidebar({
  facets,
  params,
}: {
  facets: Facets | undefined;
  params: SearchParams;
}) {
  const selectedCategory = typeof params.category === "string" ? params.category : "";
  const selectedBrand = typeof params.brand === "string" ? params.brand : "";
  const selectedPrice =
    typeof params.min_price === "string" || typeof params.max_price === "string"
      ? `${params.min_price ?? "0"}-${params.max_price ?? "+"}`
      : "";
  const selectedSort = (typeof params.sort === "string" ? params.sort : "relevance") as SortKey;

  return (
    <aside className="space-y-6 text-sm">
      <FacetGroup title="Sort">
        <ul className="space-y-1">
          {SORT_OPTIONS.map((s) => (
            <li key={s.value}>
              <Link
                href={toQuery(params, { sort: s.value === "relevance" ? undefined : s.value })}
                className={cn(
                  "block rounded px-2 py-1",
                  s.value === selectedSort
                    ? "bg-accent text-accent-foreground font-medium"
                    : "text-muted-foreground hover:bg-muted",
                )}
              >
                {s.label}
              </Link>
            </li>
          ))}
        </ul>
      </FacetGroup>

      <FacetGroup title="Category">
        <FacetList
          buckets={facets?.categories ?? []}
          selectedValue={selectedCategory}
          hrefFor={(v) =>
            toQuery(params, { category: v === selectedCategory ? undefined : v })
          }
        />
      </FacetGroup>

      <FacetGroup title="Brand">
        <FacetList
          buckets={facets?.brands ?? []}
          selectedValue={selectedBrand}
          hrefFor={(v) => toQuery(params, { brand: v === selectedBrand ? undefined : v })}
        />
      </FacetGroup>

      <FacetGroup title="Price">
        <ul className="space-y-1">
          {PRICE_RANGES.map((r) => {
            const active = r.value === selectedPrice;
            return (
              <li key={r.value}>
                <Link
                  href={toQuery(params, {
                    min_price: active ? undefined : r.min != null ? String(r.min) : undefined,
                    max_price: active ? undefined : r.max != null ? String(r.max) : undefined,
                  })}
                  className={cn(
                    "block rounded px-2 py-1",
                    active
                      ? "bg-accent text-accent-foreground font-medium"
                      : "text-muted-foreground hover:bg-muted",
                  )}
                >
                  {r.label}
                </Link>
              </li>
            );
          })}
        </ul>
      </FacetGroup>

      <FacetGroup title="Min rating">
        <ul className="space-y-1">
          {[4.5, 4.0, 3.5].map((rating) => {
            const active = params.min_rating === String(rating);
            return (
              <li key={rating}>
                <Link
                  href={toQuery(params, { min_rating: active ? undefined : String(rating) })}
                  className={cn(
                    "block rounded px-2 py-1",
                    active
                      ? "bg-accent text-accent-foreground font-medium"
                      : "text-muted-foreground hover:bg-muted",
                  )}
                >
                  ★ {rating.toFixed(1)}+
                </Link>
              </li>
            );
          })}
        </ul>
      </FacetGroup>
    </aside>
  );
}

function FacetGroup({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section>
      <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground mb-2">{title}</h3>
      {children}
    </section>
  );
}

function FacetList({
  buckets,
  selectedValue,
  hrefFor,
}: {
  buckets: { value: string; count: number }[];
  selectedValue: string;
  hrefFor: (v: string) => string;
}) {
  if (buckets.length === 0) {
    return <p className="text-xs text-muted-foreground">No values</p>;
  }
  return (
    <ul className="space-y-1">
      {buckets.map((b) => {
        const active = b.value.toLowerCase() === selectedValue.toLowerCase();
        return (
          <li key={b.value}>
            <Link
              href={hrefFor(b.value)}
              className={cn(
                "flex items-center justify-between rounded px-2 py-1 capitalize",
                active
                  ? "bg-accent text-accent-foreground font-medium"
                  : "text-muted-foreground hover:bg-muted",
              )}
            >
              <span>{b.value}</span>
              <span className="text-xs">{b.count}</span>
            </Link>
          </li>
        );
      })}
    </ul>
  );
}
