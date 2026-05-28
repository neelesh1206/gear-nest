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

const API_BASE = process.env.NEXT_PUBLIC_API_BASE ?? "";
const USE_MOCKS = !API_BASE || process.env.NEXT_PUBLIC_USE_MOCKS === "1";

async function fetchJson<T>(path: string): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, { next: { revalidate: 30 } });
  if (!res.ok) throw new Error(`API ${path} → ${res.status}`);
  return (await res.json()) as T;
}

export async function searchProducts(params: ProductSearchParams = {}): Promise<ProductSearchResponse> {
  if (USE_MOCKS) return mockSearchProducts(params);
  const search = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v != null && v !== "") search.set(k, String(v));
  }
  return fetchJson<ProductSearchResponse>(`/api/v1/products/search?${search.toString()}`);
}

export async function getProductBySlug(slug: string): Promise<ProductDetail | null> {
  if (USE_MOCKS) return mockGetProductBySlug(slug);
  return fetchJson<ProductDetail>(`/api/v1/products/${encodeURIComponent(slug)}`).catch(() => null);
}

export async function getProductPrices(id: string): Promise<PriceComparisonResponse | null> {
  if (USE_MOCKS) return mockGetProductPrices(id);
  return fetchJson<PriceComparisonResponse>(`/api/v1/products/${id}/prices`).catch(() => null);
}

export async function getProductReviews(id: string): Promise<ReviewBreakdown | null> {
  if (USE_MOCKS) return mockGetProductReviews(id);
  return fetchJson<ReviewBreakdown>(`/api/v1/products/${id}/reviews`).catch(() => null);
}

export function chatSseUrl(query: string, productId: string): string {
  const base = API_BASE || "";
  const u = new URL(`${base || "http://localhost:8080"}/api/v1/chat`);
  u.searchParams.set("query", query);
  u.searchParams.set("productId", productId);
  return base ? `${base}${u.pathname}${u.search}` : u.toString();
}

export const IS_USING_MOCKS = USE_MOCKS;
