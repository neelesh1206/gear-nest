# AGENTS — Cross-Service Warnings

## Next.js 16 — App Router breaking changes

Async `params` and `searchParams` in route handlers. Both must be awaited:

```ts
// app/products/[slug]/page.tsx
export default async function Page({
  params,
  searchParams,
}: {
  params: Promise<{ slug: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { slug } = await params;
  const sp = await searchParams;
  // ...
}
```

`cookies()`, `headers()`, and `draftMode()` from `next/headers` are also async.

## Tailwind CSS v4

No `tailwind.config.ts`. Configure via `@theme` directive in `globals.css`. Dark mode requires `@custom-variant dark (&:where(.dark, .dark *))` in the same file.

## React 19

Server Components are default. `'use client'` only for browser APIs, event handlers, or hooks. Form actions accept async functions directly; no `useFormState` wrapper needed (it became `useActionState`).

## Spring Boot 3 + Java 21

Virtual threads enabled via `spring.threads.virtual.enabled=true` in `application.yml`. SSE endpoints use `SseEmitter`; spawn work on `Thread.ofVirtual().start(...)` — do not block the Tomcat worker.

## Rust async

`async fn` in traits requires `#[async_trait]` (still standard for trait dispatch as of toolchain pin in `rust-toolchain.toml`). `tokio::spawn` for fire-and-forget; `JoinSet` for fan-out with backpressure.

## pgvector

Operator `<=>` returns cosine distance (1 - similarity). Lower is closer. Do not confuse with `<->` (L2) or `<#>` (negative inner product).

```sql
SELECT * FROM review_chunks
WHERE product_id = $1
ORDER BY embedding <=> $2::vector
LIMIT 20;
```

No HNSW indexes on chunk tables — see ADR-001.
