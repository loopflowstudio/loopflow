# 02: Contract Hardening

## Problem

Pass 1 split large modules, but core contracts are still implicit in too many places. Prompt assembly policy can drift across gather/budget/format logic, SQL query coverage can drift across SQLite/Postgres, docker recovery depends on unstated label/mount conventions, and store trait impls still hide backend `match` branching.

This hurts three groups now:
- Maintainers: refactors remain high-risk because invariants are not explicit.
- Contributors: behavior is hard to predict without reading multiple files.
- Users: regressions appear in recovery, prompt quality, and cross-backend behavior.

Why now: Pass 3+ expansion depends on stable contracts. If Pass 2 does not harden these seams, later feature work will amplify drift.

## Approach

Ship Pass 2 as a contract-first hardening lane with five concrete tracks:

1. **Prompt pipeline contracts (newtype stage boundaries).**
   Wrap `PromptComponents` in newtypes: `GatheredContext(PromptComponents)` → `BudgetedContext(PromptComponents, ContextBreakdown)` → `RenderedPrompt(String)`. Same underlying data, compiler-enforced stage ordering. Change the signatures of `gather_context`, `trim_context_with_breakdown`, and `format_prompt` to consume and produce the newtypes. Keep current behavior — the newtypes prevent passing raw `PromptComponents` across stage boundaries, not restructuring the data. Snapshot-test parity for built-in steps via golden-file tests (Track 5).

2. **SQL catalog invariant tests (completeness + correctness).**
   `catalog.rs` already centralizes 68 queries with dialect rendering. Add `#[test]` coverage that enforces:
   - every `Query` enum variant has a definition (no dead variants, no missing defs),
   - placeholder indices are contiguous and correctly numbered for both dialects,
   - dialect overrides parse without error,
   - the rendered SQL for both SQLite and Postgres is non-empty for every query.
   No `build.rs` codegen needed — the catalog is already the source of truth. Tests enforce it stays complete.

3. **Recovery/workspace invariant test suite (convention to contract).**
   Extract docker metadata string literals into named constants in `docker/mod.rs`:
   - Label keys: `LABEL_MANAGED`, `LABEL_KIND`, `LABEL_AGENT_ID`, `LABEL_WAVE_ID`, `LABEL_WAVE_RUN_ID`
   - Naming prefixes: `CONTAINER_PREFIX_AGENT`, `CONTAINER_PREFIX_PREP`, `VOLUME_PREFIX`
   - Label values: `LABEL_KIND_REPO_VOLUME`

   Replace all raw string literals in `image.rs`, `io.rs`, `recovery.rs`, and `mod.rs` with these constants. Then assert through focused tests:
   - startup cleanup and orphan handling use `LABEL_MANAGED` consistently,
   - reattach expectations match label keys set during creation,
   - workspace branch-resolution precedence,
   - ephemeral cleanup guarantees,
   - label/mount parity across `image`, `workspace`, and `recovery` modules.

4. **Store backend dispatch reduction (optional, lowest priority).**
   The backend `match` dispatch is already consolidated in `mod.rs` — every trait method follows the same `match &self.backend` pattern. This is verbose (~50 methods) but centralized and greppable. Options: introduce a `dispatch!` macro to reduce boilerplate, or leave as-is. Not an architectural extraction — the "single switch point" already exists. Deprioritize below Tracks 1-3 and 5.

5. **Conformance + quality guardrails.**
   - **API subset checks:** assert lfd HTTP semantics remain a subset of lfdhub where endpoints overlap. `cargo test api_subset`.
   - **Golden prompt tests:** snapshot the fully rendered prompt for critical built-in steps (e.g., `implement`, `review`, `debug`) and diff against known-good outputs. `cargo test golden_prompt`. Deterministic file-based snapshots, not LLM-as-judge evals.

This directly advances wave goals: **"Maintain architectural compactness as features grow," "Eliminate boilerplate and duplicated patterns,"** and **"Make extension points trait-based, not switch-based."**

## Implementation sequence

Track 3 (docker label constants + invariant tests) → Track 2 (catalog invariant tests) → Track 4 (store dispatch, if pursued) → Track 1 (prompt pipeline newtypes) → Track 5 (golden prompt tests + API subset checks).

Rationale: Tracks 3 and 2 are independent foundations with no cross-dependencies. Track 1 touches the prompt pipeline; Track 5 snapshots its output — so 5 must follow 1. Track 4 is optional and lowest priority.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Patch regressions case-by-case without structural contract work | Lowest short-term cost | Keeps drift as the default mode; slows every future change |
| Full abstraction rewrite (new trait hierarchies everywhere) | Potentially clean conceptual model | High over-decomposition risk; violates infra warning from Pass 1 |
| Adopt external compile-time frameworks for SQL/API contracts immediately | Strong static guarantees | High migration overhead now; larger blast radius than targeted hardening |
| Distinct structs instead of newtypes for prompt stages | Richer type-level guarantees | Requires splitting `PromptComponents` fields across types; high refactor cost in a 2,730-line file for marginal benefit over newtypes |
| `build.rs` codegen for SQL catalog | Compile-time failure | `catalog.rs` already centralizes queries; `#[test]` coverage achieves the same guarantee with less machinery |
| Store backend-port adapter traits (`SqliteStoreBackend` / `PostgresStoreBackend`) | Clean polymorphism | Dispatch is already consolidated in one file; adapter traits add indirection without reducing the actual switch point count |

## Key decisions

- **Choose explicit contracts over inferred behavior.** Every high-risk seam gets typed handoffs or invariant tests.
- **Prefer test-time validation over build-time codegen** where the source of truth already exists (catalog.rs, label constants).
- **Keep decomposition pragmatic.** Newtypes over new structs. Constants over trait abstractions. Tests over code generation.
- **Wild success target:** contributors can modify prompt/store/recovery paths with confidence because breakage is caught by deterministic tests before runtime.
- **Wild failure to avoid:** six months later we discover recovery regressions caused by undocumented metadata conventions and scattered backend branching; this design prevents that by centralizing conventions and codifying invariants.
- **New risk introduced:** test-suite sprawl can slow iteration. Mitigation: keep invariant tests focused on boundary contracts, not internal wiring.

## Scope

- In scope:
  - Prompt gather/budget/format newtype wrappers with snapshot parity
  - SQL catalog completeness + placeholder invariant tests
  - Recovery/workspace/store parity invariants
  - Docker label/mount constants and contract assertions across lifecycle modules
  - Store backend dispatch macro reduction (optional, lowest priority)
  - lfd↔lfdhub API subset checks and golden prompt snapshot tests
- Out of scope:
  - New trigger types
  - New flow language features
  - Major HTTP API redesign
  - New provider capability rollout

## Done when

- `cargo fmt --all -- --check`
- `cargo clippy -p loopflow --all-targets -- -D warnings`
- `cargo test -p loopflow prompt`
- `cargo test -p loopflow store`
- `cargo test -p loopflow recovery`
- `cargo test -p loopflow api_subset`
- `cargo test -p loopflow golden_prompt`

And all of these are observably true:
- Prompt stage boundaries are newtype-enforced and parity-tested via golden snapshots.
- SQL catalog completeness and placeholder correctness are assertion-backed.
- Recovery/workspace metadata conventions use named constants and are assertion-backed.
- Store backend dispatch is consolidated (macro-reduced if pursued, otherwise documented as intentionally verbose).
- API subset + golden prompt checks run in CI for critical paths.
