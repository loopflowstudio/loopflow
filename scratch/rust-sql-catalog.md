# 03: Shared SQL Catalog

## Problem

`lfd` currently duplicates the same query logic across `sqlite.rs` and `postgres.rs`, with only placeholder syntax and driver calls changing. That duplication slows every schema change, invites silent backend drift, and makes review harder because behavior lives in two files.

Who benefits: maintainers shipping store changes, reviewers validating correctness, and users who rely on both SQLite and Postgres behaving identically.

Why now: Stage 02 already unified call sites behind async `Store`; SQL duplication is now the largest remaining source of storage-layer complexity.

## Approach

Build a single query catalog and make backends consume it, not author SQL inline.

1. Add `lfd/store/catalog.rs` with one `Query` entry per operation (wave CRUD, run status updates, schema/history reads, hooks, repos, etc.).
2. Author SQL once in a dialect-neutral template with numbered placeholders (`{p1}`, `{p2}`, ...).
3. Render templates into backend SQL via a tiny dialect adapter:
   - SQLite: `{pN}` -> `?N`
   - Postgres: `{pN}` -> `$N`
4. Support true syntax differences explicitly with per-query overrides (rare, named, reviewed), not ad-hoc backend-local SQL strings.
5. Cache rendered SQL once (lazy static) so queries do not pay replacement cost on hot paths.
6. Add catalog parity tests that assert every `Query` renders for both dialects and that placeholder numbering is contiguous.
7. Refactor `sqlite.rs` and `postgres.rs` to call catalog queries; delete duplicated inline SQL.

Wild success shape: adding a new store operation means touching one catalog entry plus backend binding code, and both backends ship together by default.

Wild failure shape (to avoid): catalog becomes a hidden DSL with unreadable indirection. Keep it plain strings + tiny renderer + explicit overrides.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep SQL duplicated, enforce parity with reviewer discipline | Lowest migration effort | Fails at scale; drift is process debt, not design |
| Adopt full query builder/ORM (Diesel, SeaQuery, SQLx macro-heavy rewrite) | Stronger compile-time modeling | Too disruptive for this stage; large rewrite risk and slower iteration |
| Generate Rust constants from `.sql` files per backend | Better SQL ergonomics for editors | Still duplicates backend variants and adds build tooling complexity |

## Key decisions

- Keep **both** backends first-class, following wave north star: **"lfd keeps both SQLite and Postgres."**
- Delete replaced patterns in the same stage, following wave rule: **"Each stage deletes what it replaces — no deferred cleanup."**
- Implement exactly the Stage 03 contract from wave plan: **"One query per operation; dialect rendering for `?`/`$N`."**
- Prefer explicit per-query dialect overrides over clever abstraction when SQL diverges (readability beats magic).
- Treat catalog parity tests as a release gate for store changes.

## Scope

- In scope:
  - New shared SQL catalog module and placeholder renderer
  - Refactor duplicated queries out of `sqlite.rs` and `postgres.rs`
  - Catalog parity tests and regression coverage for store behavior
- Out of scope:
  - Replacing rusqlite/tokio-postgres drivers
  - Changing wave/execution product semantics
  - Splitting executor modules or redesigning `Store` dispatch API

## Done when

- Every store operation SQL statement is defined once in `lfd/store/catalog.rs`.
- `sqlite.rs` and `postgres.rs` no longer contain duplicated inline query bodies (except driver-specific transaction/plumbing code).
- Catalog tests verify both dialect renderings and placeholder correctness.
- Store + integration behavior still passes:
  - `cargo test -p loopflow lfd::store`
  - `cargo test -p loopflow lfd::http`
  - `cargo test -p loopflow lfd::executor`
