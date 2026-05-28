"use server";

import { searchProducts } from "@/lib/api/client";
import type { ProductCard } from "@/lib/api/types";

export interface SearchSuggestion {
  id: string;
  slug: string;
  name: string;
  brand: string;
  category: string;
}

export async function searchSuggestions(query: string): Promise<SearchSuggestion[]> {
  const q = query.trim();
  if (q.length < 2) return [];
  const result = await searchProducts({ q, size: 6 });
  return result.products.map(toSuggestion);
}

function toSuggestion(p: ProductCard): SearchSuggestion {
  return {
    id: p.id,
    slug: p.slug,
    name: p.name,
    brand: p.brand,
    category: p.category,
  };
}
