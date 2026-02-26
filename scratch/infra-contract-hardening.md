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

1. **Prompt pipeline contracts (typed stage boundaries).**
   Introduce explicit handoff types for GatheredPromptContext → BudgetedPromptContext → RenderedPrompt. Keep current behavior by snapshot-testing parity for built-in steps while moving policy decisions into one stage each.

2. **Build-time SQL catalog validation (single source of truth).**
   Generate a query catalog index at build time (same pattern already proven in `build.rs` direction discovery), then validate:
   - required query IDs exist for both dialects,
   - placeholders are dialect-correct,
   - malformed definitions fail compile/test early.

3. **Recovery/workspace invariant test suite (convention to contract).**
   Promote docker metadata conventions into named constants and assert them through focused tests:
   - startup cleanup and orphan handling,
   - reattach expectations,
   - workspace branch-resolution precedence,
   - ephemeral cleanup guarantees,
   - label/mount parity across `image`, `workspace`, and `recovery` modules.

4. **Store backend-port adapters (single backend switch point).**
   Extract remaining backend `match` logic from trait impls into adapter surfaces (`SqliteStoreBackend` / `PostgresStoreBackend` or equivalent macro-reduced adapters). Callers use capability traits only.

5. **Conformance + quality guardrails.**
   Add API subset checks (lfd semantics remain a subset of lfdhub where overlapping) and add prompt quality-axis evals for critical steps (security/reliability/performance/api/ux), tied to prompt bundle/version outcomes.

This directly advances wave goals: **“Maintain architectural compactness as features grow,” “Eliminate boilerplate and duplicated patterns,”** and **“Make extension points trait-based, not switch-based.”**

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Patch regressions case-by-case without structural contract work | Lowest short-term cost | Keeps drift as the default mode; slows every future change |
| Full abstraction rewrite (new trait hierarchies everywhere) | Potentially clean conceptual model | High over-decomposition risk; violates infra warning from Pass 1 |
| Adopt external compile-time frameworks for SQL/API contracts immediately | Strong static guarantees | High migration overhead now; larger blast radius than targeted hardening |

## Key decisions

- **Choose explicit contracts over inferred behavior.** Every high-risk seam gets typed handoffs or invariant tests.
- **Prefer generated validation over manual review.** Build/test failure is the gate, not tribal knowledge.
- **Keep decomposition pragmatic.** Extract backend ports where they reduce repeated `match`, but avoid micro-trait explosion.
- **Wild success target:** contributors can modify prompt/store/recovery paths with confidence because breakage is caught by deterministic tests before runtime.
- **Wild failure to avoid:** six months later we discover recovery regressions caused by undocumented metadata conventions and scattered backend branching; this design prevents that by centralizing conventions and codifying invariants.
- **New risk introduced:** test-suite sprawl can slow iteration. Mitigation: keep invariant tests focused on boundary contracts, not internal wiring.

## Scope

- In scope:
  - Prompt gather/budget/format stage separation with output parity
  - Build-time SQL catalog completeness + placeholder validation
  - Recovery/workspace/store parity invariants
  - Docker label/mount contract assertions across lifecycle modules
  - Store backend-port dispatch consolidation
  - lfd↔lfdhub API subset checks and prompt quality eval harness
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
- Prompt stage boundaries are explicit and parity-tested.
- SQL catalog drift fails at build/test time.
- Recovery/workspace metadata conventions are assertion-backed.
- Store trait impls no longer contain scattered backend `match` dispatch.
- API subset + prompt quality checks run in CI for critical paths.
