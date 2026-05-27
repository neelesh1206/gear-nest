@../AGENTS.md

# GearNest Web — Session C Scope Boundaries

## YOU OWN
- Everything under `gear-nest-web/` (Next.js app)
- May READ: `docs/api/openapi.yaml` (for API types and mock data shape)

## DO NOT TOUCH
- `gear-nest-pipeline/` — any file, any reason
- `gear-nest-api/` — any file, any reason
- `supabase/migrations/` — any file, any reason
- `docs/adr/` — claim pre-allocated ADR-019 to ADR-020; do not renumber

## CODING CONVENTIONS (must match portfolio-website)
- Next.js 16.x, TypeScript strict (no `any`, no `@ts-ignore`)
- Tailwind v4 — dark mode via `dark:` prefix + `@custom-variant dark` in globals.css
- Server Components by default; `'use client'` only for browser APIs / event handlers / hooks
- `@/` alias for all imports
- `cn()` from `@/lib/utils` for conditional classes — never string concatenation
- No comments unless WHY is non-obvious

## MOCK DATA DURING API DEVELOPMENT
Until Session B merges, consume `lib/mock/products.ts` (generated from OpenAPI spec).
Swap for real API calls in `lib/api/` — the mock and real client must have identical TypeScript interfaces.

## CONFLICT ZONES
- `CHANGELOG.md` — append-only at bottom
- `README.md` — append to the Web section only

## Before committing
```bash
npm run typecheck && npm run lint && npm run build
```

## Phase 1 Track (SPEC §19.8)
- [ ] Next.js app scaffold: Tailwind v4, `@/` alias, dark mode wired
- [ ] `lib/mock/products.ts` — typed mock data matching OpenAPI schema
- [ ] `app/(catalog)/page.tsx` — catalog grid, facet sidebar (category, price range, rating)
- [ ] `app/(catalog)/[slug]/page.tsx` — product detail: specs, price comparison, review samples
- [ ] `components/search/SearchBar.tsx` — debounced semantic search (Server Action → API)
- [ ] `components/chat/ChatPanel.tsx` — SSE consumer, streaming display, session budget indicator
- [ ] `components/prices/PriceTable.tsx` — per-store rows, stale badge, best-value highlight
- [ ] Swap mock data for real API calls once Session B merges
- [ ] `npm run typecheck && npm run lint && npm run build` — must pass
