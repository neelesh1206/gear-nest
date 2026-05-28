export type MatchConfidence = "EXACT" | "HIGH" | "MEDIUM" | "CANDIDATE";

export type SortKey = "best_value" | "price_asc" | "rating_desc" | "relevance";

export interface Store {
  id: string;
  displayName: string;
  logoUrl?: string | null;
}

export interface AggregateRating {
  average: number;
  count: number;
}

export interface FacetBucket {
  value: string;
  count: number;
}

export interface Facets {
  brands: FacetBucket[];
  categories: FacetBucket[];
  priceRanges: FacetBucket[];
}

export interface ProductCard {
  id: string;
  slug: string;
  name: string;
  brand: string;
  category: string;
  subcategory?: string | null;
  primaryImage?: string | null;
  aggregateRating?: AggregateRating;
  lowestPrice?: number | null;
  currency?: string;
}

export interface StoreListing {
  id: string;
  store: Store;
  storeProductId: string;
  storeUrl: string;
  affiliateUrl?: string | null;
  price?: number | null;
  currency?: string;
  inStock?: boolean | null;
  storeRating?: number | null;
  reviewCount: number;
  matchConfidence: MatchConfidence;
  bestValueScore?: number | null;
  isBestValue: boolean;
  priceFetchedAt?: string | null;
  isStale: boolean;
}

export interface ProductDetail {
  id: string;
  slug: string;
  name: string;
  brand: string;
  category: string;
  subcategory?: string | null;
  description?: string | null;
  specs?: Record<string, string | number | boolean>;
  images: string[];
  aggregateRating?: AggregateRating;
  listings: StoreListing[];
  pricesLastUpdated?: string | null;
  pricesNextUpdate?: string | null;
}

export interface ProductSearchResponse {
  products: ProductCard[];
  total: number;
  page: number;
  size: number;
  facets?: Facets;
}

export interface PriceComparisonResponse {
  listings: StoreListing[];
  lastUpdated?: string | null;
  nextUpdate?: string | null;
}

export interface Review {
  id: string;
  rating: number;
  title?: string | null;
  body: string;
  verifiedPurchase: boolean;
  helpfulVotes: number;
  reviewDate?: string | null;
  store: Store;
}

export interface ReviewTier {
  count: number;
  sample: Review[];
}

export interface ReviewBreakdown {
  tiers: {
    "1"?: ReviewTier;
    "2"?: ReviewTier;
    "3"?: ReviewTier;
    "4"?: ReviewTier;
    "5"?: ReviewTier;
  };
  total: number;
  storeBreakdown?: { store: Store; count: number; avgRating: number }[];
}

export interface AiSummary {
  summary: string;
  pros: string[];
  cons: string[];
  reviewCount: number;
  generatedAt: string;
}

export interface ProductSearchParams {
  q?: string;
  category?: string;
  brand?: string;
  min_price?: number;
  max_price?: number;
  sort?: SortKey;
  page?: number;
  size?: number;
}

export type ChatEvent =
  | { type: "token"; data: string }
  | { type: "done"; data: { remaining: number; sessionId: string } }
  | { type: "limit_reached"; data: Record<string, never> }
  | { type: "error"; data: { budgetRestored: boolean } };
