import Link from "next/link";
import { notFound } from "next/navigation";
import type { Metadata } from "next";
import { ChatPanel } from "@/components/chat/ChatPanel";
import { PriceTable } from "@/components/prices/PriceTable";
import { ReviewTierList } from "@/components/reviews/ReviewTierList";
import { getProductBySlug, getProductPrices, getProductReviews } from "@/lib/api/client";

type RouteParams = { slug: string };

export async function generateMetadata({
  params,
}: {
  params: Promise<RouteParams>;
}): Promise<Metadata> {
  const { slug } = await params;
  const product = await getProductBySlug(slug);
  if (!product) return { title: "Not found" };
  return {
    title: `${product.brand} ${product.name}`,
    description: product.description ?? undefined,
  };
}

export default async function ProductDetailPage({
  params,
}: {
  params: Promise<RouteParams>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { slug } = await params;
  const product = await getProductBySlug(slug);
  if (!product) notFound();

  const [prices, reviews] = await Promise.all([
    getProductPrices(product.id),
    getProductReviews(product.id),
  ]);

  return (
    <div className="space-y-8">
      <nav className="text-xs text-muted-foreground">
        <Link href="/" className="hover:underline">Catalog</Link>
        <span className="mx-1">›</span>
        <span className="capitalize">{product.category}</span>
        <span className="mx-1">›</span>
        <span>{product.brand}</span>
      </nav>

      <header className="grid grid-cols-1 lg:grid-cols-[1.2fr_1fr] gap-8">
        <div className="aspect-[4/3] rounded-lg bg-muted flex items-center justify-center text-muted-foreground">
          {product.images[0] ? (
            <img src={product.images[0]} alt={product.name} className="h-full w-full rounded-lg object-cover" />
          ) : (
            <span className="text-sm">{product.brand}</span>
          )}
        </div>
        <div className="space-y-3">
          <p className="text-xs uppercase tracking-wide text-muted-foreground">{product.brand}</p>
          <h1 className="text-2xl font-semibold tracking-tight">{product.name}</h1>
          {product.aggregateRating ? (
            <p className="text-sm text-muted-foreground">
              ★ {product.aggregateRating.average.toFixed(1)} ·{" "}
              {product.aggregateRating.count.toLocaleString()} reviews
            </p>
          ) : null}
          {product.description ? (
            <p className="text-sm leading-relaxed">{product.description}</p>
          ) : null}
          {product.specs ? (
            <dl className="mt-2 grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
              {Object.entries(product.specs).map(([k, v]) => (
                <div key={k} className="contents">
                  <dt className="text-muted-foreground capitalize">{k.replace(/([A-Z])/g, " $1").trim()}</dt>
                  <dd className="font-medium">{String(v)}</dd>
                </div>
              ))}
            </dl>
          ) : null}
        </div>
      </header>

      <PriceTable
        listings={prices?.listings ?? product.listings}
        lastUpdated={prices?.lastUpdated ?? product.pricesLastUpdated}
        nextUpdate={prices?.nextUpdate ?? product.pricesNextUpdate}
      />

      <div className="grid grid-cols-1 lg:grid-cols-[1.6fr_1fr] gap-8">
        {reviews ? <ReviewTierList breakdown={reviews} /> : null}
        <ChatPanel productId={product.id} productName={product.name} />
      </div>
    </div>
  );
}
