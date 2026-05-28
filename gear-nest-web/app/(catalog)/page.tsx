import { Suspense } from "react";
import { searchProducts } from "@/lib/api/client";
import { FacetSidebar } from "@/components/catalog/FacetSidebar";
import { ProductCard } from "@/components/catalog/ProductCard";
import { SearchBar } from "@/components/search/SearchBar";
import type { ProductSearchParams, SortKey } from "@/lib/api/types";

type RawSearchParams = Record<string, string | string[] | undefined>;

function asString(v: string | string[] | undefined): string | undefined {
  if (Array.isArray(v)) return v[0];
  return v;
}

function asNumber(v: string | string[] | undefined): number | undefined {
  const s = asString(v);
  if (!s) return undefined;
  const n = Number(s);
  return Number.isFinite(n) ? n : undefined;
}

function asSort(v: string | string[] | undefined): SortKey | undefined {
  const s = asString(v);
  if (s === "best_value" || s === "price_asc" || s === "rating_desc" || s === "relevance") return s;
  return undefined;
}

export default async function CatalogPage({
  searchParams,
}: {
  searchParams: Promise<RawSearchParams>;
}) {
  const sp = await searchParams;
  const params: ProductSearchParams = {
    q: asString(sp.q),
    category: asString(sp.category),
    brand: asString(sp.brand),
    min_price: asNumber(sp.min_price),
    max_price: asNumber(sp.max_price),
    sort: asSort(sp.sort),
    page: asNumber(sp.page),
    size: asNumber(sp.size),
  };
  const minRating = asNumber(sp.min_rating);

  const result = await searchProducts(params);
  const products = minRating != null
    ? result.products.filter((p) => (p.aggregateRating?.average ?? 0) >= minRating)
    : result.products;

  return (
    <div className="grid grid-cols-1 lg:grid-cols-[16rem_1fr] gap-8">
      <FacetSidebar facets={result.facets} params={sp} />
      <section className="space-y-4">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <h1 className="text-2xl font-semibold tracking-tight">
            {params.q ? `Results for "${params.q}"` : "All gear"}
          </h1>
          <Suspense fallback={null}>
            <SearchBar initialQuery={params.q ?? ""} />
          </Suspense>
        </div>
        <p className="text-sm text-muted-foreground">
          {products.length} {products.length === 1 ? "product" : "products"} · sorted by {params.sort ?? "relevance"}
        </p>
        {products.length === 0 ? (
          <p className="rounded-md border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
            Nothing matches these filters yet. Try clearing brand or price range.
          </p>
        ) : (
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
            {products.map((p) => (
              <ProductCard key={p.id} product={p} />
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
