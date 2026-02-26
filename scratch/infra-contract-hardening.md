# 02: Contract hardening

Pass 2 hardens implicit contracts that were still causing drift risk after core-boundary cleanup.
This file is the single source for scope, outcomes, and remaining follow-ups.

## Goals

Codify cross-module contracts so prompt assembly, Docker lifecycle behavior, and store SQL handling
cannot silently drift.

Specifically:
- enforce prompt stage ordering at the type level,
- make Docker metadata conventions explicit and shared,
- prove SQL catalog completeness/correctness across SQLite and Postgres,
- keep quality guardrails deterministic via API subset and golden prompt tests.

## Implemented on this branch

### Prompt pipeline contracts

- Added explicit stage types in `engine/prompt.rs`:
  - `GatheredContext(PromptComponents)`
  - `BudgetedContext(PromptComponents, ContextBreakdown)`
  - `RenderedPrompt(String)`
- Updated prompt call sites (`engine/launch.rs`, `ops/lint.rs`, `ops/rebase.rs`,
  `bin/lf-prompt.rs`, tests) to follow gather → budget → render ordering.
- Preserved prompt behavior; this change is contract hardening, not policy redesign.

### Docker lifecycle contract hardening

- Split the old monolithic executor into:
  - `docker/mod.rs` (orchestration + shared constants)
  - `docker/image.rs`
  - `docker/workspace.rs`
  - `docker/recovery.rs`
  - `docker/io.rs`
- Centralized Docker metadata conventions as named constants
  (managed labels, kind labels, container/volume prefixes).
- Replaced scattered string literals with shared constants across modules.
- Added focused invariant tests in `docker/tests.rs` for:
  - label/prefix contract usage,
  - startup rehydration and orphan cleanup behavior,
  - workspace branch-resolution precedence,
  - mount/volume/credential expectations.

### Store SQL contract hardening

- Added catalog-wide invariants in `lfd/store/catalog.rs` to ensure:
  - every `Query` variant has a definition,
  - rendered SQL is non-empty for both SQLite and Postgres,
  - placeholders are contiguous/correct for templates and dialect overrides,
  - overrides remain renderable.

### Conformance guardrails

- Added/updated golden prompt fixtures:
  - `tests/goldens/builtin_implement.{yaml,md}`
  - `tests/goldens/builtin_review.{yaml,md}`
  - `tests/goldens/builtin_debug.{yaml,md}`
- Kept store backend dispatch explicit and centralized in `lfd/store/mod.rs`
  (documented as intentional, not accidental boilerplate).

## Decisions and rationale

- **Newtypes over new structs** for prompt stages: lower refactor cost, same ordering guarantees.
- **Invariant tests over codegen** for SQL catalog: existing catalog is already the source of truth.
- **Constants over convention-by-literal** in Docker modules: single metadata vocabulary.
- **Explicit store dispatch retained**: one greppable backend switch point, no macro indirection for now.

## Validation status

Validated locally:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow prompt`
- `cargo test -p loopflow store`
- `cargo test -p loopflow recovery`
- `cargo test -p loopflow api_subset`
- `cargo test -p loopflow golden_prompt`

## Remaining follow-ups

- `engine/prompt.rs` is still large; stage contracts are explicit, but concentration risk remains.
- Store dispatch remains intentionally verbose; revisit macro reduction only if churn cost grows.
- Docker recovery correctness still benefits from daemon-restart end-to-end coverage beyond unit tests.
