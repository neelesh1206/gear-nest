import Link from "next/link";
import type { ProductCard as ProductCardData } from "@/lib/api/types";
import { cn, formatCurrency } from "@/lib/utils";

export function ProductCard({ product }: { product: ProductCardData }) {
  return (
    <Link
      href={`/${product.slug}`}
      className={cn(
        "group flex flex-col rounded-lg border border-border bg-card text-card-foreground",
        "p-4 transition hover:border-foreground/30 hover:shadow-sm",
      )}
    >
      <div className="aspect-[4/3] w-full rounded-md bg-muted mb-3 flex items-center justify-center text-muted-foreground text-xs">
        {product.primaryImage ? (
          <img
            src={product.primaryImage}
            alt={product.name}
            className="h-full w-full object-cover rounded-md"
          />
        ) : (
          <span aria-hidden>{product.brand}</span>
        )}
      </div>
      <div className="text-xs uppercase tracking-wide text-muted-foreground">{product.brand}</div>
      <h3 className="font-medium leading-tight mt-1 line-clamp-2 group-hover:underline">{product.name}</h3>
      <div className="mt-auto pt-3 flex items-baseline justify-between">
        <span className="text-base font-semibold">{formatCurrency(product.lowestPrice, product.currency)}</span>
        {product.aggregateRating ? (
          <span className="text-xs text-muted-foreground">
            ★ {product.aggregateRating.average.toFixed(1)} ({product.aggregateRating.count.toLocaleString()})
          </span>
        ) : null}
      </div>
    </Link>
  );
}
