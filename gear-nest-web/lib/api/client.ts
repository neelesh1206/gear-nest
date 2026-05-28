import {
  getProductBySlug as mockGetProductBySlug,
  getProductPrices as mockGetProductPrices,
  getProductReviews as mockGetProductReviews,
  searchProducts as mockSearchProducts,
} from "@/lib/mock/products";
import type {
  PriceComparisonResponse,
  ProductDetail,
  ProductSearchParams,
  ProductSearchResponse,
  ReviewBreakdown,
} from "@/lib/api/types";

const API_BASE = process.env.NEXT_PUBLIC_API_BASE ?? "http://localhost:8080";
const USE_MOCKS = process.env.NEXT_PUBLIC_USE_MOCKS === "1";

async function fetchJson<T>(path: string): Promise<T | null> {
  let res: Response;
  try {
    res = await fetch(`${API_BASE}${path}`, { next: { revalidate: 30 } });
  } catch {
    return null;
  }
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`API ${path} → ${res.status}`);
  return (await res.json()) as T;
}

export async function searchProducts(params: ProductSearchParams = {}): Promise<ProductSearchResponse> {
  if (USE_MOCKS) return mockSearchProducts(params);
  const search = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v != null && v !== "") search.set(k, String(v));
  }
  const result = await fetchJson<ProductSearchResponse>(`/api/v1/products/search?${search.toString()}`);
  return result ?? { products: [], total: 0, page: params.page ?? 1, size: params.size ?? 24 };
}

export async function getProductBySlug(slug: string): Promise<ProductDetail | null> {
  if (USE_MOCKS) return mockGetProductBySlug(slug);
  return fetchJson<ProductDetail>(`/api/v1/products/${encodeURIComponent(slug)}`);
}

export async function getProductPrices(id: string): Promise<PriceComparisonResponse | null> {
  if (USE_MOCKS) return mockGetProductPrices(id);
  return fetchJson<PriceComparisonResponse>(`/api/v1/products/${id}/prices`);
}

export async function getProductReviews(id: string): Promise<ReviewBreakdown | null> {
  if (USE_MOCKS) return mockGetProductReviews(id);
  return fetchJson<ReviewBreakdown>(`/api/v1/products/${id}/reviews`);
}

export function chatSseUrl(query: string, productId: string): string {
  const u = new URL(`${API_BASE}/api/v1/chat`);
  u.searchParams.set("query", query);
  u.searchParams.set("productId", productId);
  return u.toString();
}

export const IS_USING_MOCKS = USE_MOCKS;
