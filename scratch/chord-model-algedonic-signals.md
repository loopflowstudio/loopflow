# Algedonic Signals: Live Demo

## Problem

The algedonic signal system is built — repair lineage, error classification, escalation, attention creation, Concerto display. But it hasn't run end-to-end. Three infra gaps block the live demo:

1. **Dev lfd token isolation** — dev lfd and Concerto lfd fight over `~/.lf/session-token`. Can't run both.
2. **PR state sync** — after `ops: land --create-pr`, the run's snapshot doesn't reflect the new PR. CI failure webhooks need a PR to target.
3. **Retry limit** — the design calls for 3 repair attempts, but `execute_run_inner` dispatches exactly one repair. A second failure escalates immediately.

Fixing these and building a demo harness proves the system works.

## Approach

Three surgical changes plus a demo script.

### 1. `LF_HOME` environment variable

`lf_home_dir()` in `rust/loopflow/src/lfd/mod.rs:177` checks `LF_HOME` before falling back to `~/.lf`. One line change. `session-token`, `lfd.db`, and all state files follow.

```rust
pub(crate) fn lf_home_dir() -> PathBuf {
    if let Ok(home) = std::env::var("LF_HOME") {
        return PathBuf::from(home);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lf")
}
```

`scripts/dev-lfq` and the demo script use `LF_HOME=/tmp/lfd-dev` (or a temp dir) to isolate completely.

### 2. PR state sync after land

After `ops: land` creates a PR, write the PR back to the run's snapshot. The change lives in the land handler (`rust/loopflow/src/lfd/http/routes/waves.rs:1155`). After `land()` returns with a PR URL/number, update the run's `snapshot.pr` in the store.

The `WaveRunSnapshot` is meant to be immutable (captured at run start), but this is a special case: the run *created* the PR. The snapshot should reflect the run's own output. Alternative: store PR number outside the snapshot (a new field on `WaveRun`). But the snapshot's `pr` field already exists and is what downstream code reads. Updating it post-land is simpler and more correct — the snapshot captures the run's state, and the PR is part of that state.

### 3. Retry limit (3 attempts)

Current behavior: first failure → one repair attempt → escalate. The design says 3.

Add a store query: count runs where `repair_of` traces back to the same original failure (follow the chain). In `execute_run_inner`, before dispatching repair, check the count. If >= 3, create algedonic signal directly instead of another repair run.

Implementation: `count_repair_chain(store, &run)` walks `repair_of` links backwards to count depth. Simple and correct — no new fields needed.

### 4. Demo harness script

`scripts/demo-algedonic.sh` orchestrates the full path:

1. Build lfd (`cargo build -p loopflow --bin lfd`)
2. Start isolated lfd (`LF_HOME=$(mktemp -d)`)
3. Create wave with `make-tests-fail` → `land --create-pr` flow
4. Run wave — step breaks a test, pushes PR
5. Simulate CI failure event (POST to webhook endpoint)
6. Wait for ci-fix run to appear
7. If ci-fix fails → verify repair run dispatched (repair_of set)
8. If repair fails → verify algedonic attention item created
9. Print results, cleanup

Uses `LfdRuntime` pattern from `scripts/lib/lfd_runtime.py` — proven to work.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| `HOME` env var instead of `LF_HOME` | Isolates everything, not just lfd state | Too broad — breaks git, ssh, cargo. `LF_HOME` is surgical. |
| New `WaveRun.pr_number` field instead of updating snapshot | Cleaner separation of mutable/immutable | Downstream code reads `snapshot.pr`. Adding another field means updating all readers. Updating snapshot is one write. |
| Retry limit as a config field | Configurable per wave | Premature. Start with a constant. Make it configurable when someone needs it. |
| Polling-based CI check instead of webhook | No webhook infra needed | Slower, wasteful. Webhook path already exists. Demo can simulate the webhook. |

## Key decisions

1. **`LF_HOME` over `HOME` override.** Scoped to lfd state only. Doesn't break the rest of the system.

2. **Update snapshot.pr post-land.** Yes, this mutates an "immutable" snapshot. But the alternative (a new field + updating all readers) is worse. The snapshot should reflect reality, and reality includes the PR that the run created.

3. **Retry count by walking `repair_of` chain, not a counter field.** No schema change. The chain *is* the count. Slightly more expensive (N queries for depth N, max 3), but N is tiny and correctness is obvious.

4. **Demo simulates CI webhook rather than waiting for real CI.** Real CI takes minutes and requires GitHub. The demo should run locally in seconds. Real CI integration is already tested via the webhook path.

## Scope

- In scope:
  - `LF_HOME` env var in `lf_home_dir()`
  - PR state sync after `ops: land --create-pr`
  - Retry limit (3) with chain depth counting
  - `scripts/demo-algedonic.sh` orchestration script
  - Update `scripts/dev-lfq` to use `LF_HOME`
  - Tests for retry limit and PR sync

- Out of scope:
  - Concerto interactive debug launch (separate feature, already spec'd)
  - Real GitHub CI integration (webhook path exists, demo simulates)
  - Configurable retry limit
  - Auto-resolve on CI success (already works via `reconcile_attention_items` — step failure resolves when a newer run supersedes the failed one)

## Done when

```bash
# Demo runs end-to-end
scripts/demo-algedonic.sh

# Output shows:
# 1. Wave created
# 2. Run executed (step fails)
# 3. Repair run dispatched (repair_of links to failed run)
# 4. After 3 failed repairs, algedonic attention item created
# 5. Attention item visible via GET /attention
```

- `LF_HOME=/tmp/test cargo test` passes — lfd uses isolated state
- `cargo test repair_chain` — retry limit works at depth 3
- `cargo test pr_sync_after_land` — snapshot.pr updated after land
- `scripts/demo-algedonic.sh` completes without errors
