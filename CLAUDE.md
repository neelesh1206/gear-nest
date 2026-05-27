# GearNest — Root Conventions

This file applies to all sessions working in this monorepo. Per-service files extend it:
- [`gear-nest-pipeline/CLAUDE.md`](./gear-nest-pipeline/CLAUDE.md) — Rust
- [`gear-nest-api/CLAUDE.md`](./gear-nest-api/CLAUDE.md) — Java + Spring Boot
- [`gear-nest-web/CLAUDE.md`](./gear-nest-web/CLAUDE.md) — Next.js + TypeScript

Authoritative spec: [`SPEC.md`](./SPEC.md). Parallel session rules: [`SPEC.md` §19](./SPEC.md#19-parallel-implementation-guide-claude-code-cli).

---

## Scope Boundaries (Critical)

Each service track owns its directory exclusively. Cross-service edits cause merge conflicts and break the parallel-session model.

| Track | Owns | Never Touch |
|-------|------|-------------|
| Session 0 (Contract) | `docs/`, `supabase/migrations/`, root config files, per-service `CLAUDE.md` | Any service `src/` |
| Session A (Pipeline) | `gear-nest-pipeline/` | `gear-nest-api/`, `gear-nest-web/` |
| Session B (API)      | `gear-nest-api/`      | `gear-nest-pipeline/`, `gear-nest-web/` |
| Session C (Web)      | `gear-nest-web/`      | `gear-nest-pipeline/`, `gear-nest-api/` |

---

## Shared Conflict Zones (SPEC §19.7)

| File | Rule |
|------|------|
| `CHANGELOG.md` | Append-only. One line at the bottom per change. Never rewrite or reorder. Format: `YYYY-MM-DD · <service> · <feature> — one-line summary`. |
| `README.md` | Sectioned. Edit only your service's section. |
| `supabase/migrations/` | Session 0 only. Propose schema changes via PR comment. |
| `docker-compose.yml` | Session 0 only. New dependencies need sign-off. |
| `docs/adr/` | ADR numbers pre-allocated: 013-015 Pipeline, 016-018 API, 019-020 Web. Write content, never renumber. |

---

## Code Discipline (applies everywhere)

- No comments unless WHY is non-obvious — a hidden constraint, a subtle invariant, a workaround. Don't explain WHAT; identifiers and types do that.
- No backwards-compatibility shims, no removed-code marker comments, no unused-var renames.
- Don't add error handling, fallbacks, or validation for scenarios that can't happen. Validate only at system boundaries.
- Edit existing files in preference to creating new ones.
- Do not create documentation files unless explicitly requested.

---

## Working Across Sessions

1. Read `CLAUDE.md` in your worktree root — it defines your scope.
2. Read `SPEC.md` §16 (Build Phases) and §19 (Parallel Guide).
3. Read the last few `CHANGELOG.md` entries to know what shipped.
4. `git log --oneline -10` on your branch — your recent commits.
5. Pick up the next unchecked item in your track's list in §19.8.

Never ask "what should I work on?" — the checklist is the answer.
