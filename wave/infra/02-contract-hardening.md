# 02: Contract Hardening

Stabilize core contracts after boundary cleanup so prompt/store/recovery behavior is predictable and testable.

## Why this phase exists

After pass 1 decomposition, the next failure mode is contract drift:

- prompt assembly policy is still intertwined across gather/budget/format stages
- SQL catalog growth increases dialect/placeholder drift risk
- recovery/workspace paths need stronger invariant coverage

Contract hardening keeps feature velocity without hidden regressions.

## Scope

### In scope

1. **Prompt pipeline decomposition**
   - Separate gather → budget/trim → format stages with explicit data handoff types.
   - Preserve output parity for current flows/steps.
2. **SQL catalog validation**
   - Add build-time checks for query coverage and placeholder sanity across SQLite/Postgres.
   - Fail fast on missing or malformed query definitions.
3. **Invariant-focused test expansion**
   - Recovery invariants (startup cleanup, orphan handling, reattach expectations).
   - Workspace invariants (branch resolution precedence, ephemeral cleanup contract).
   - Store parity invariants (critical behavior matches across SQLite/Postgres).
4. **API subset conformance checks**
   - Assert lfd endpoint semantics remain a subset of lfdhub public API semantics where features overlap.
   - Mark any local-only lfd endpoints explicitly.
5. **Prompt quality-eval harness**
   - Add quality-axis evals (security/reliability/performance/api/ux) for critical prompt steps.
   - Track prompt bundle/version outcomes and guard against regressions.

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
- lfd↔lfdhub API subset conformance checks are explicit and passing.
- Prompt quality eval/golden checks are explicit and passing for critical steps.
