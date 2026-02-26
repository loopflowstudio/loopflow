# 02: Contract Hardening

Stabilize core contracts after boundary cleanup so prompt/store/recovery behavior is predictable and testable.

## Why this phase exists

After Pass 1 decomposition, the next failure mode is contract drift:

- prompt assembly policy is still intertwined across gather/budget/format stages
- SQL catalog growth increases dialect/placeholder drift risk
- recovery/workspace paths need stronger invariant coverage — Pass 1 revealed that docker recovery correctness depends on implicit label/mount conventions across the new lifecycle modules
- store backend `match` dispatch still exists inside trait impls (Pass 1 added capability accessors but didn't extract backend-port adapters)

Contract hardening keeps feature velocity without hidden regressions.

## Scope

### In scope

1. **Prompt pipeline decomposition**
   - Separate gather → budget/trim → format stages with explicit data handoff types.
   - Preserve output parity for current flows/steps.
2. **SQL catalog validation**
   - Add build-time checks for query coverage and placeholder sanity across SQLite/Postgres.
   - Fail fast on missing or malformed query definitions.
   - Pattern proven by direction taxonomy: `build.rs` scans directories, generates `LazyLock<HashMap>`, validated at compile time. Apply the same approach to SQL catalog.
3. **Invariant-focused test expansion**
   - Recovery invariants (startup cleanup, orphan handling, reattach expectations). Higher priority than originally estimated — Pass 1 decomposition spread recovery-relevant conventions across `docker/recovery.rs`, `docker/workspace.rs`, and `docker/image.rs`. Label and mount conventions need explicit assertion coverage.
   - Workspace invariants (branch resolution precedence, ephemeral cleanup contract).
   - Store parity invariants (critical behavior matches across SQLite/Postgres).
   - Docker startup-recovery tests that currently soft-skip without Docker — decide whether to split into a Docker-required suite or keep soft-skipping.
4. **API subset conformance checks**
   - Assert lfd endpoint semantics remain a subset of lfdhub public API semantics where features overlap.
   - Mark any local-only lfd endpoints explicitly.
5. **Prompt quality-eval harness**
   - Quality directions (`infra/`, `ux/`, `craft/`, `ceo/`) now provide concrete eval axes.
   - Add quality-axis evals (security/reliability/performance/api/ux) for critical prompt steps.
   - Track prompt bundle/version outcomes and guard against regressions.

6. **Store backend-port cleanup** (carried from Pass 1)
   - Remaining backend `match` dispatch inside store trait impls can be extracted into backend-port adapters (`SqliteStoreBackend` / `PostgresStoreBackend`) or reduced via macro generation.
   - Goal: callers interact only through capability traits; backend selection is a single decision point, not scattered `match` blocks.

### Out of scope

- New trigger types
- New flow language features
- Major HTTP API redesign
- New provider capability rollout

## Contract

- Prompt output remains behaviorally compatible for existing built-in steps/flows.
- SQL catalog remains single source of truth; checks increase confidence without changing API semantics.
- Recovery and workspace behavior become assertion-backed at boundary points.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy -p loopflow --all-targets -- -D warnings`
- `cargo test -p loopflow prompt`
- `cargo test -p loopflow store`
- `cargo test -p loopflow recovery`
- `cargo test -p loopflow api_subset`
- `cargo test -p loopflow golden_prompt`

## Done when

- Prompt pipeline has clear stage boundaries and stable outputs.
- SQL catalog fails at build/test time for coverage/placeholder drift.
- Recovery/workspace/store parity invariants have explicit tests and pass in CI.
- Docker label/mount conventions are assertion-backed across lifecycle modules.
- Store backend dispatch is consolidated (no scattered `match` blocks in trait impls).
- lfd↔lfdhub API subset conformance checks are explicit and passing.
- Prompt quality eval/golden checks are explicit and passing for critical steps.
