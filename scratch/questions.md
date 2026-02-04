# Open Questions

Questions that emerged during lfd-primary implementation. Need resolution before or during next iteration.

## Architecture

- **RunWave overrides**: `flow`, `direction`, `area` overrides currently persist to `Wave`. Design says wave config shouldn't change. Should we add override fields to `WaveRun` instead?

## Scheduler slots

- **Watch/cron bypass slots**: Loop ticker acquires scheduler slots before creating WaveRun, but watch/cron triggers start runs without slot checks. Should they also acquire/release scheduler slots?
- **Interactive resumes skip slot checks**: ConnectWave/EndAgent resumes call executor directly without acquiring scheduler slots. Should interactive resumes also acquire/release slots?

## Deferred features

- **Choose/Fork selection**: `ForkSelect::One` picks first option deterministically (no LLM). `ForkSelect::Prompt` also picks first. When should we wire a choice agent?
- **Fork retry tracking**: Design has `fork_attempts` placeholder but it's not implemented. Add when we hit transient failures in practice.

## WaveService Protocol

- Should `/wave-runs` move to a `/v1/` endpoint or keep legacy-style `ok/result` responses long-term?
- Should `WaveServiceFactory` select contexts from config (grpc/remote) instead of always returning `LocalWaveService`?

## Concerto Wave Phase Status

**Observation**: The Phase 1 ordered set in `roadmap/concerto/README.md` references items that don't exist:
- `20260131-02-history-and-recency.md`
- `20260131-03-waiting-state-actionable.md`
- `20260131-04-running-state-progress-and-connect.md`
- `20260131-05-empty-state-creates-and-teaches.md`
- `20260131-06-quick-experiment-path.md`

Recent commits suggest items 04 and 06 have shipped. Item 01 is noted as complete in the README.

**Question**: Is Phase 1 complete? Should we:
1. Move to Phase 2 (Remote access foundation)?
2. Update the README to reflect Phase 1 completion?
3. Create the missing Phase 1 items if they're not actually done?

The remaining items in `roadmap/concerto/` are all Phase 2 or 3.

## Auth Implementation

- Refresh endpoint response: assumed /auth/refresh returns JSON {token|jwt} or raw token string; confirm actual contract.
- Package supports macOS 15 only; design doc targets macOS 14+/iOS 17+. If iOS support is required, update Swift package platforms and add iOS app target.
- RemoteWaveService not present in repo; TokenProvider added but not wired into any remote service yet.

## lf ops (Rust)

1. **Rebase assistant prompt**: Rust ops uses a custom inline prompt instead of a dedicated `rebase` step file. There is no Rust builtin `rebase` step today. Should we add builtin ops steps to `loopflow-engine` and switch to them?
2. **Next branch naming**: `lf ops next` still uses `next-<timestamp>` instead of wave-based branch naming (no wave metadata update in Rust yet). Should we wire it to wave naming once the wave module is ported?
3. **Land local strategy**: `lf ops land --local` uses squash merge via `git::land` and skips PR-related steps. Confirm if we need a local merge option or to support `--strategy` explicitly.
4. **Lint fixer**: lint retry uses the built-in `lint` step via prompt context, not `lf lint -b` subprocess. Is that preferred for parity?

## Prompt parity tests

- **Flow coverage**: Parity tests use `lf run <step> --dry-run` even in the `with-flow` fixture because Rust `lf` doesn't resolve flows yet. Should we add flow execution to Rust CLI (or switch the test to `lf flow` once parity exists)?

## Rebase blocked (2026-02-04)

Rebase onto main aborted due to complex conflicts. Main merged `loopflow-ops` in PR #267, and this branch has a parallel implementation.

**Conflict summary:**
- 8 add/add conflicts in `rust/loopflow-ops/src/*.rs` (both branches added these files)
- 1 conflict in `rust/lf/src/commands/ops/mod.rs`

**Unique to this branch (valuable, not on main):**

| Content | Files | Value |
|---------|-------|-------|
| Parity test infrastructure | `tests/parity/` | High - enables prompt comparison |
| `--dry-run` flag | `rust/lf/src/` + `src/loopflow/lf/step.py` | High - enables parity testing |
| Roadmap doc updates | `roadmap/rust/01-lf-ops-parity.md` | Medium - documentation |

**Commits to cherry-pick (in order):**
```
be7096be6 Add prompt parity tests and dry-run output
013aaa921 Simplify runner checks and parity git helpers
d4538c83d lf consolidate: parity tests: add initial fixture set
ad1acb3a4 lf lint: tests: normalize quotes in parity fixture
```

**Commits to skip (already on main via PR #267):**
```
cf6c3e64a lf implement: loopflow-ops: add workflow orchestration crate
721d46372 lf gate: loopflow-ops: add workflow orchestration crate
```

**Recommended approach:**
1. Create fresh branch from main: `git checkout -b parity-tests origin/main`
2. Cherry-pick parity test commits: `git cherry-pick be7096be6 013aaa921 d4538c83d ad1acb3a4`
3. Resolve any minor conflicts (likely just import paths)
4. Abandon this branch after cherry-pick succeeds
