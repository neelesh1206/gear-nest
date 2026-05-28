import type {
  PriceComparisonResponse,
  ProductCard,
  ProductDetail,
  ProductSearchParams,
  ProductSearchResponse,
  Review,
  ReviewBreakdown,
  StoreListing,
} from "@/lib/api/types";
import { STORES } from "@/lib/mock/stores";

const NOW = new Date("2026-05-27T15:00:00Z").toISOString();
const HOURS_AGO = (h: number) =>
  new Date(Date.parse("2026-05-27T15:00:00Z") - h * 3_600_000).toISOString();

function mkListing(
  id: string,
  storeId: keyof typeof STORES,
  storeProductId: string,
  price: number | null,
  options: Partial<StoreListing> = {},
): StoreListing {
  return {
    id,
    store: STORES[storeId],
    storeProductId,
    storeUrl: `https://${storeId}.example.com/${storeProductId}`,
    affiliateUrl: null,
    price,
    currency: "USD",
    inStock: true,
    storeRating: 4.5,
    reviewCount: 0,
    matchConfidence: "EXACT",
    bestValueScore: null,
    isBestValue: false,
    priceFetchedAt: HOURS_AGO(4),
    isStale: false,
    ...options,
  };
}

function mkReview(
  id: string,
  rating: 1 | 2 | 3 | 4 | 5,
  body: string,
  storeId: keyof typeof STORES,
  options: Partial<Review> = {},
): Review {
  return {
    id,
    rating,
    title: null,
    body,
    verifiedPurchase: true,
    helpfulVotes: 12,
    reviewDate: "2026-04-15",
    store: STORES[storeId],
    ...options,
  };
}

interface ProductMock {
  detail: ProductDetail;
  reviews: ReviewBreakdown;
}

export const PRODUCTS: ProductMock[] = [
  {
    detail: {
      id: "11111111-1111-1111-1111-111111111111",
      slug: "msr-pocketrocket-2",
      name: "MSR PocketRocket 2 Stove",
      brand: "MSR",
      category: "camping",
      subcategory: "stoves",
      description:
        "Ultralight canister stove that boils a liter of water in 3.5 minutes. Folds to fit in a 100g fuel canister cup.",
      specs: {
        weight: "73 g",
        boilTime: "3.5 min / L",
        fuel: "isobutane / propane canister",
        packedSize: "fits in 100g canister cup",
        material: "stainless / aluminum",
      },
      images: [],
      aggregateRating: { average: 4.8, count: 2143 },
      listings: [
        mkListing("a-1", "rei", "REI-msr-pr2", 49.95, {
          storeRating: 4.9,
          reviewCount: 812,
          isBestValue: true,
          bestValueScore: 0.92,
        }),
        mkListing("a-2", "amazon", "B07ABCD", 47.99, {
          storeRating: 4.4,
          reviewCount: 1102,
          bestValueScore: 0.86,
        }),
        mkListing("a-3", "backcountry", "BC-msr-pr2", 49.95, {
          storeRating: 4.7,
          reviewCount: 121,
          bestValueScore: 0.81,
          isStale: true,
          priceFetchedAt: HOURS_AGO(29),
        }),
        mkListing("a-4", "moosejaw", "MJ-msr-pr2", 54.0, {
          storeRating: 4.6,
          reviewCount: 88,
          bestValueScore: 0.62,
        }),
        mkListing("a-5", "campsaver", "CS-msr-pr2", 52.5, {
          storeRating: 4.5,
          reviewCount: 20,
          bestValueScore: 0.59,
          matchConfidence: "HIGH",
        }),
        mkListing("a-6", "ggg", "GGG-msr-candidate", 51.0, {
          matchConfidence: "CANDIDATE",
          bestValueScore: 0.4,
        }),
      ],
      pricesLastUpdated: HOURS_AGO(4),
      pricesNextUpdate: HOURS_AGO(-20),
    },
    reviews: {
      total: 2143,
      tiers: {
        "5": {
          count: 1620,
          sample: [
            mkReview(
              "r-5a",
              5,
              "Boils water faster than my home stove. I've used it in 20°F mornings with a windscreen and it never sputters. Worth every penny.",
              "rei",
            ),
            mkReview(
              "r-5b",
              5,
              "Backpacked the Wind Rivers with this. 73g and bombproof — replaced my old Whisperlite and never looked back.",
              "backcountry",
            ),
          ],
        },
        "4": {
          count: 380,
          sample: [
            mkReview(
              "r-4a",
              4,
              "Solid stove. Knock half a star because the flame control is finicky for actual cooking — fine for boiling.",
              "amazon",
            ),
            mkReview(
              "r-4b",
              4,
              "Works great. Wish it came with the case but that's nitpicking.",
              "rei",
            ),
          ],
        },
        "3": {
          count: 90,
          sample: [
            mkReview(
              "r-3a",
              3,
              "Performance drops noticeably below 25°F. Fine for 3-season but I'd reach for white gas in winter.",
              "amazon",
            ),
          ],
        },
        "2": {
          count: 38,
          sample: [
            mkReview(
              "r-2a",
              2,
              "Pot supports are too narrow — my 1.3L pot tips constantly. Returning.",
              "moosejaw",
            ),
          ],
        },
        "1": {
          count: 15,
          sample: [
            mkReview(
              "r-1a",
              1,
              "Mine arrived with a bent jet. MSR support replaced it but the QC clearly slipped.",
              "amazon",
            ),
          ],
        },
      },
      storeBreakdown: [
        { store: STORES.rei, count: 812, avgRating: 4.9 },
        { store: STORES.amazon, count: 1102, avgRating: 4.4 },
        { store: STORES.backcountry, count: 121, avgRating: 4.7 },
      ],
    },
  },
  {
    detail: {
      id: "22222222-2222-2222-2222-222222222222",
      slug: "arcteryx-beta-ar-jacket",
      name: "Arc'teryx Beta AR Jacket",
      brand: "Arc'teryx",
      category: "apparel",
      subcategory: "shells",
      description:
        "Three-layer GORE-TEX Pro shell designed for all-round alpine use. StormHood with helmet compatibility, watertight pit zips.",
      specs: {
        material: "GORE-TEX Pro 3L",
        weight: "490 g",
        hood: "StormHood (helmet-compatible)",
        seams: "fully taped",
        warranty: "limited lifetime",
      },
      images: [],
      aggregateRating: { average: 4.7, count: 612 },
      listings: [
        mkListing("b-1", "backcountry", "BC-arc-beta-ar", 600.0, {
          storeRating: 4.7,
          reviewCount: 180,
          isBestValue: true,
          bestValueScore: 0.78,
        }),
        mkListing("b-2", "rei", "REI-arc-beta-ar", 650.0, {
          storeRating: 4.9,
          reviewCount: 240,
          bestValueScore: 0.71,
        }),
        mkListing("b-3", "moosejaw", "MJ-arc-beta-ar", 619.0, {
          storeRating: 4.6,
          reviewCount: 41,
          bestValueScore: 0.66,
          isStale: true,
          priceFetchedAt: HOURS_AGO(27),
        }),
        mkListing("b-4", "amazon", "B09XYZ", 670.0, {
          storeRating: 4.3,
          reviewCount: 88,
          bestValueScore: 0.55,
          matchConfidence: "HIGH",
        }),
      ],
      pricesLastUpdated: HOURS_AGO(3),
      pricesNextUpdate: HOURS_AGO(-21),
    },
    reviews: {
      total: 612,
      tiers: {
        "5": {
          count: 420,
          sample: [
            mkReview(
              "r2-5a",
              5,
              "Wore this through a five-day Patagonia traverse — sideways rain, sleet, wind. Bone dry. The hood is the best on any shell I've owned.",
              "backcountry",
            ),
            mkReview(
              "r2-5b",
              5,
              "Worth the price if you actually live in mountains. Cuts wind better than my old Beta SL.",
              "rei",
            ),
          ],
        },
        "4": {
          count: 130,
          sample: [
            mkReview(
              "r2-4a",
              4,
              "Excellent shell but I expected articulated cuffs to be more refined at this price.",
              "rei",
            ),
          ],
        },
        "3": {
          count: 40,
          sample: [
            mkReview(
              "r2-3a",
              3,
              "Performance is great. Fit runs slim — size up if you layer heavily.",
              "moosejaw",
            ),
          ],
        },
        "2": {
          count: 15,
          sample: [
            mkReview(
              "r2-2a",
              2,
              "Pit zip seam failed at 18 months. Arc'teryx warranty replaced it, but for $650 I expected longer.",
              "backcountry",
            ),
          ],
        },
        "1": {
          count: 7,
          sample: [
            mkReview(
              "r2-1a",
              1,
              "Wetted out in heavy rain after one season. DWR clearly degraded fast.",
              "amazon",
            ),
          ],
        },
      },
    },
  },
  {
    detail: {
      id: "33333333-3333-3333-3333-333333333333",
      slug: "rab-mythic-ultra-180",
      name: "Rab Mythic Ultra 180 Sleeping Bag",
      brand: "Rab",
      category: "camping",
      subcategory: "sleeping-bags",
      description:
        "900FP hydrophobic goose-down sleeping bag rated to -7 °C. TILT baffles concentrate fill above the body for warmth-to-weight gains.",
      specs: {
        fill: "900FP hydrophobic goose down",
        limitTemp: "-7 °C",
        weight: "650 g",
        fillWeight: "180 g",
        shell: "Atmos 7D",
      },
      images: [],
      aggregateRating: { average: 4.6, count: 187 },
      listings: [
        mkListing("c-1", "rei", "REI-rab-mu180", 700.0, {
          storeRating: 4.8,
          reviewCount: 64,
          isBestValue: true,
          bestValueScore: 0.74,
        }),
        mkListing("c-2", "ggg", "GGG-rab-mu180", 695.0, {
          storeRating: 4.9,
          reviewCount: 14,
          bestValueScore: 0.73,
        }),
        mkListing("c-3", "backcountry", "BC-rab-mu180", 725.0, {
          storeRating: 4.7,
          reviewCount: 22,
          bestValueScore: 0.6,
        }),
      ],
      pricesLastUpdated: HOURS_AGO(6),
      pricesNextUpdate: HOURS_AGO(-18),
    },
    reviews: {
      total: 187,
      tiers: {
        "5": {
          count: 124,
          sample: [
            mkReview(
              "r3-5a",
              5,
              "Slept warm at 22 °F in a single base layer. Lofts huge for the weight.",
              "rei",
            ),
            mkReview(
              "r3-5b",
              5,
              "Replaced my heavier 20°F bag and gained an entire pound back. Game-changer.",
              "ggg",
            ),
          ],
        },
        "4": {
          count: 38,
          sample: [
            mkReview(
              "r3-4a",
              4,
              "Warm and light. Zipper snags more than I'd like for the price.",
              "backcountry",
            ),
          ],
        },
        "3": {
          count: 15,
          sample: [
            mkReview(
              "r3-3a",
              3,
              "Cold for me at the rated temp — I sleep cold so size up your buffer.",
              "rei",
            ),
          ],
        },
        "2": {
          count: 7,
          sample: [
            mkReview(
              "r3-2a",
              2,
              "Down clumped after the first wash even with the recommended technique.",
              "rei",
            ),
          ],
        },
        "1": {
          count: 3,
          sample: [
            mkReview(
              "r3-1a",
              1,
              "Stitching unraveled at the foot box after three trips. Returned.",
              "backcountry",
            ),
          ],
        },
      },
    },
  },
  {
    detail: {
      id: "44444444-4444-4444-4444-444444444444",
      slug: "hoka-speedgoat-5",
      name: "HOKA Speedgoat 5 Trail Runner",
      brand: "HOKA",
      category: "footwear",
      subcategory: "trail-running",
      description:
        "Maximally cushioned trail runner with Vibram MegaGrip outsole. Ideal for technical mountain trails and ultras.",
      specs: {
        weight: "286 g (M9)",
        drop: "4 mm",
        outsole: "Vibram MegaGrip",
        stack: "33/29 mm",
      },
      images: [],
      aggregateRating: { average: 4.5, count: 932 },
      listings: [
        mkListing("d-1", "amazon", "B0Speedgoat", 140.0, {
          storeRating: 4.6,
          reviewCount: 540,
          isBestValue: true,
          bestValueScore: 0.83,
        }),
        mkListing("d-2", "rei", "REI-hoka-sg5", 155.0, {
          storeRating: 4.9,
          reviewCount: 312,
          bestValueScore: 0.69,
        }),
        mkListing("d-3", "backcountry", "BC-hoka-sg5", 150.0, {
          storeRating: 4.7,
          reviewCount: 41,
          bestValueScore: 0.66,
        }),
        mkListing("d-4", "steepcheap", "SC-hoka-sg5", 109.0, {
          storeRating: 4.4,
          reviewCount: 11,
          bestValueScore: 0.88,
          isStale: true,
          priceFetchedAt: HOURS_AGO(30),
        }),
      ],
      pricesLastUpdated: HOURS_AGO(2),
      pricesNextUpdate: HOURS_AGO(-22),
    },
    reviews: {
      total: 932,
      tiers: {
        "5": {
          count: 600,
          sample: [
            mkReview(
              "r4-5a",
              5,
              "Ran a 50k in these straight out of the box. Grip on wet roots is unreal.",
              "amazon",
            ),
            mkReview(
              "r4-5b",
              5,
              "My third pair. Sole grip stays consistent through ~500 miles.",
              "rei",
            ),
          ],
        },
        "4": {
          count: 220,
          sample: [
            mkReview(
              "r4-4a",
              4,
              "Great shoe, but the upper started fraying around the toe at 200 miles.",
              "rei",
            ),
          ],
        },
        "3": {
          count: 70,
          sample: [
            mkReview(
              "r4-3a",
              3,
              "Wide forefoot but the heel runs narrow — needed double socks for security.",
              "backcountry",
            ),
          ],
        },
        "2": {
          count: 30,
          sample: [
            mkReview(
              "r4-2a",
              2,
              "Lugs wore through faster than I expected for a Vibram outsole.",
              "amazon",
            ),
          ],
        },
        "1": {
          count: 12,
          sample: [
            mkReview(
              "r4-1a",
              1,
              "Heel collar collapsed after a wet 30-miler. Disappointing for the price.",
              "amazon",
            ),
          ],
        },
      },
    },
  },
  {
    detail: {
      id: "55555555-5555-5555-5555-555555555555",
      slug: "garmin-fenix-7-sapphire",
      name: "Garmin Fenix 7 Sapphire Solar",
      brand: "Garmin",
      category: "electronics",
      subcategory: "watches",
      description:
        "Multi-sport GPS watch with solar charging, topographic maps, and 18-day battery life in smartwatch mode.",
      specs: {
        display: "1.3\" sapphire touchscreen",
        battery: "18 d smartwatch mode",
        gnss: "multi-band",
        weight: "73 g",
      },
      images: [],
      aggregateRating: { average: 4.7, count: 1404 },
      listings: [
        mkListing("e-1", "amazon", "B0Fenix7", 799.0, {
          storeRating: 4.5,
          reviewCount: 980,
          isBestValue: true,
          bestValueScore: 0.81,
        }),
        mkListing("e-2", "rei", "REI-fenix-7", 899.0, {
          storeRating: 4.9,
          reviewCount: 84,
          bestValueScore: 0.65,
        }),
        mkListing("e-3", "backcountry", "BC-fenix-7", 879.0, {
          storeRating: 4.7,
          reviewCount: 20,
          bestValueScore: 0.62,
        }),
      ],
      pricesLastUpdated: HOURS_AGO(1),
      pricesNextUpdate: HOURS_AGO(-23),
    },
    reviews: {
      total: 1404,
      tiers: {
        "5": {
          count: 980,
          sample: [
            mkReview(
              "r5-5a",
              5,
              "Battery still has 40% after a 12-day rim-to-rim Grand Canyon trip with full GPS.",
              "amazon",
            ),
            mkReview(
              "r5-5b",
              5,
              "Topo maps preloaded for the entire US. Replaced my old Garmin handheld entirely.",
              "rei",
            ),
          ],
        },
        "4": {
          count: 280,
          sample: [
            mkReview(
              "r5-4a",
              4,
              "Killer watch — solar charging is a real feature, not marketing. Touchscreen sometimes laggy with wet hands.",
              "rei",
            ),
          ],
        },
        "3": {
          count: 90,
          sample: [
            mkReview(
              "r5-3a",
              3,
              "Pulse-ox readings during sleep are wildly variable. GPS itself is excellent.",
              "amazon",
            ),
          ],
        },
        "2": {
          count: 35,
          sample: [
            mkReview(
              "r5-2a",
              2,
              "Software updates regularly introduce new bugs. Stability has gone backward in 2026.",
              "amazon",
            ),
          ],
        },
        "1": {
          count: 19,
          sample: [
            mkReview(
              "r5-1a",
              1,
              "Bezel paint flaked after 4 months. Returned under warranty but second unit had the same issue.",
              "amazon",
            ),
          ],
        },
      },
    },
  },
  {
    detail: {
      id: "66666666-6666-6666-6666-666666666666",
      slug: "patagonia-nano-puff-hoody",
      name: "Patagonia Nano Puff Hoody",
      brand: "Patagonia",
      category: "apparel",
      subcategory: "insulation",
      description:
        "60g PrimaLoft Gold synthetic insulation in a recycled-polyester shell. Compresses to a grapefruit; warm even when damp.",
      specs: {
        insulation: "PrimaLoft Gold Eco 60 g",
        shell: "100% recycled polyester",
        weight: "337 g",
        packed: "stuff-sack pocket",
      },
      images: [],
      aggregateRating: { average: 4.6, count: 2204 },
      listings: [
        mkListing("f-1", "rei", "REI-nano-puff-hoody", 279.0, {
          storeRating: 4.9,
          reviewCount: 940,
          isBestValue: true,
          bestValueScore: 0.79,
        }),
        mkListing("f-2", "backcountry", "BC-nano-puff-hoody", 279.0, {
          storeRating: 4.7,
          reviewCount: 142,
          bestValueScore: 0.74,
        }),
        mkListing("f-3", "moosejaw", "MJ-nano-puff-hoody", 269.0, {
          storeRating: 4.6,
          reviewCount: 67,
          bestValueScore: 0.71,
        }),
        mkListing("f-4", "steepcheap", "SC-nano-puff-hoody", 195.0, {
          storeRating: 4.4,
          reviewCount: 35,
          bestValueScore: 0.91,
        }),
      ],
      pricesLastUpdated: HOURS_AGO(5),
      pricesNextUpdate: HOURS_AGO(-19),
    },
    reviews: {
      total: 2204,
      tiers: {
        "5": {
          count: 1540,
          sample: [
            mkReview(
              "r6-5a",
              5,
              "Lives in my pack year round. Light, packable, dries fast when sweat-soaked.",
              "rei",
            ),
            mkReview(
              "r6-5b",
              5,
              "Doesn't lose loft when wet — unlike my old down sweater. Perfect Pacific NW layer.",
              "backcountry",
            ),
          ],
        },
        "4": {
          count: 430,
          sample: [
            mkReview(
              "r6-4a",
              4,
              "Warm enough as a midlayer. Hood is on the small side — won't fit over a helmet.",
              "moosejaw",
            ),
          ],
        },
        "3": {
          count: 140,
          sample: [
            mkReview(
              "r6-3a",
              3,
              "Cooler than I expected at this weight. Better for spring shoulder season than winter.",
              "rei",
            ),
          ],
        },
        "2": {
          count: 60,
          sample: [
            mkReview(
              "r6-2a",
              2,
              "Stitching at the hand pockets came loose after one season.",
              "rei",
            ),
          ],
        },
        "1": {
          count: 34,
          sample: [
            mkReview(
              "r6-1a",
              1,
              "Shell tore on a brushy bushwhack — wouldn't trust it off-trail.",
              "backcountry",
            ),
          ],
        },
      },
    },
  },
];

function toCard(detail: ProductDetail): ProductCard {
  const lowest = detail.listings
    .filter((l) => l.matchConfidence !== "CANDIDATE" && l.price != null)
    .reduce<number | null>(
      (min, l) => (min == null || (l.price ?? Infinity) < min ? l.price ?? min : min),
      null,
    );
  return {
    id: detail.id,
    slug: detail.slug,
    name: detail.name,
    brand: detail.brand,
    category: detail.category,
    subcategory: detail.subcategory,
    primaryImage: detail.images[0] ?? null,
    aggregateRating: detail.aggregateRating,
    lowestPrice: lowest,
    currency: "USD",
  };
}

function matchesQuery(card: ProductCard, q?: string): boolean {
  if (!q) return true;
  const needle = q.toLowerCase();
  return (
    card.name.toLowerCase().includes(needle) ||
    card.brand.toLowerCase().includes(needle) ||
    card.category.toLowerCase().includes(needle) ||
    (card.subcategory ?? "").toLowerCase().includes(needle)
  );
}

function buildFacets(cards: ProductCard[]) {
  const brands = new Map<string, number>();
  const categories = new Map<string, number>();
  const priceBuckets: Record<string, number> = {
    "0-50": 0,
    "50-150": 0,
    "150-400": 0,
    "400+": 0,
  };
  for (const c of cards) {
    brands.set(c.brand, (brands.get(c.brand) ?? 0) + 1);
    categories.set(c.category, (categories.get(c.category) ?? 0) + 1);
    const p = c.lowestPrice ?? 0;
    if (p < 50) priceBuckets["0-50"]!++;
    else if (p < 150) priceBuckets["50-150"]!++;
    else if (p < 400) priceBuckets["150-400"]!++;
    else priceBuckets["400+"]!++;
  }
  const toBuckets = (m: Map<string, number>) =>
    [...m.entries()].sort((a, b) => b[1] - a[1]).map(([value, count]) => ({ value, count }));
  return {
    brands: toBuckets(brands),
    categories: toBuckets(categories),
    priceRanges: Object.entries(priceBuckets).map(([value, count]) => ({ value, count })),
  };
}

export async function searchProducts(params: ProductSearchParams = {}): Promise<ProductSearchResponse> {
  const cards = PRODUCTS.map((p) => toCard(p.detail));
  const brandFilter = params.brand?.split(",").map((b) => b.trim().toLowerCase());

  const filtered = cards.filter((c) => {
    if (!matchesQuery(c, params.q)) return false;
    if (params.category && c.category !== params.category) return false;
    if (brandFilter && brandFilter.length > 0 && !brandFilter.includes(c.brand.toLowerCase())) return false;
    if (params.min_price != null && (c.lowestPrice ?? 0) < params.min_price) return false;
    if (params.max_price != null && (c.lowestPrice ?? Infinity) > params.max_price) return false;
    return true;
  });

  const sort = params.sort ?? "relevance";
  filtered.sort((a, b) => {
    switch (sort) {
      case "price_asc":
        return (a.lowestPrice ?? Infinity) - (b.lowestPrice ?? Infinity);
      case "rating_desc":
        return (b.aggregateRating?.average ?? 0) - (a.aggregateRating?.average ?? 0);
      case "best_value":
        return (
          (b.aggregateRating?.average ?? 0) / Math.max(b.lowestPrice ?? 1, 1) -
          (a.aggregateRating?.average ?? 0) / Math.max(a.lowestPrice ?? 1, 1)
        );
      default:
        return 0;
    }
  });

  const size = params.size ?? 24;
  const page = params.page ?? 1;
  const start = (page - 1) * size;
  const sliced = filtered.slice(start, start + size);

  return {
    products: sliced,
    total: filtered.length,
    page,
    size,
    facets: buildFacets(cards),
  };
}

export async function getProductBySlug(slug: string): Promise<ProductDetail | null> {
  const found = PRODUCTS.find((p) => p.detail.slug === slug);
  return found ? found.detail : null;
}

export async function getProductPrices(productId: string): Promise<PriceComparisonResponse | null> {
  const found = PRODUCTS.find((p) => p.detail.id === productId);
  if (!found) return null;
  return {
    listings: found.detail.listings,
    lastUpdated: found.detail.pricesLastUpdated,
    nextUpdate: found.detail.pricesNextUpdate,
  };
}

export async function getProductReviews(productId: string): Promise<ReviewBreakdown | null> {
  const found = PRODUCTS.find((p) => p.detail.id === productId);
  return found ? found.reviews : null;
}

export const MOCK_NOW = NOW;
