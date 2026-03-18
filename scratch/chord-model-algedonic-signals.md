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

After `ops: land` creates a PR, set `run.pr` in the store. The change lives in the land handler (`rust/loopflow/src/lfd/http/routes/waves.rs:1155`). After `land()` returns with a PR URL/number, update `run.pr`.

The PR is a run output, not initial state — it belongs on `WaveRun` directly, not inside `WaveRunSnapshot`. A run *is* a snapshot of the wave; `WaveRunSnapshot` as a separate struct is redundant. Move `pr` from `snapshot.pr` to `run.pr` and update readers accordingly. This also cleans up the awkwardness of mutating something called a "snapshot."

### 3. Retry limit (3 attempts)

Current behavior: first failure → one repair attempt → escalate. The design says 3.

Add a store query: count runs where `repair_of` traces back to the same original failure (follow the chain). In `execute_run_inner`, before dispatching repair, check the count. If >= 3, create algedonic signal directly instead of another repair run.

Implementation: `count_repair_chain(store, &run)` walks `repair_of` links backwards to count depth. Simple and correct — no new fields needed.

**Restructuring required:** The current code in `execute_run_inner` (line 59) short-circuits with `if run.repair_of.is_some() { return; }` — repair runs that fail don't trigger further repairs. Escalation happens inside `fail_run` in the executor. To support 3 attempts:
- Remove the `repair_of.is_some()` early return in `execute_run_inner`
- Move algedonic signal creation out of `fail_run` for repair runs
- `execute_run_inner` handles all runs uniformly: on failure, check chain depth → if < 3, repair; if >= 3, escalate

**Backoff between retries.** Immediate retry burns attempts against transient failures (GitHub API down, network blip). Fixed delays indexed by chain depth:

```rust
const REPAIR_DELAYS: [Duration; 3] = [
    Duration::from_secs(30),
    Duration::from_secs(60),
    Duration::from_secs(120),
];
```

The agent runs themselves take minutes, so these delays are small relative to total cycle time. Hardcoded — configurable when someone needs it.

### 4. Demo harness script

`scripts/demo-algedonic.py` orchestrates the pure repair chain → escalation path:

1. Build lfd (`cargo build -p loopflow --bin lfd`)
2. Start isolated lfd (`LF_HOME=$(mktemp -d)`)
3. Create wave with a step that always fails
4. Run wave — step fails
5. Verify repair run 1 dispatched (`repair_of` links to failed run, 30s delay)
6. Repair 1 fails → verify repair run 2 dispatched (60s delay)
7. Repair 2 fails → verify repair run 3 dispatched (120s delay)
8. Repair 3 fails → verify algedonic attention item created
9. Attention item visible via GET /attention
10. Print results, cleanup

No webhook simulation, no PR, no CI. Tests the core mechanism: repeated failure → backoff → escalation. Total runtime ~4 minutes (3 delays + step execution time).

Uses `LfdRuntime` pattern from `scripts/lib/lfd_runtime.py` — proven to work.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| `HOME` env var instead of `LF_HOME` | Isolates everything, not just lfd state | Too broad — breaks git, ssh, cargo. `LF_HOME` is surgical. |
| Keep `pr` on `WaveRunSnapshot` and mutate it post-land | Fewer reader changes | Mutating a "snapshot" is a lie. The run is the snapshot; PR is a run output. Move it to `WaveRun.pr` and update readers. |
| Retry limit as a config field | Configurable per wave | Premature. Start with a constant. Make it configurable when someone needs it. |
| No backoff between retries | Simpler, fewer moving parts | Burns all 3 attempts immediately against transient failures (API down, rate limit). Fixed delays are one constant. |
| Exponential backoff with jitter | Better for high-contention systems | Over-engineered for 3 retries where each attempt takes minutes. Fixed delays are predictable and sufficient. |
| Polling-based CI check instead of webhook | No webhook infra needed | Slower, wasteful. Webhook path already exists. Demo can simulate the webhook. |

## Key decisions

1. **`LF_HOME` over `HOME` override.** Scoped to lfd state only. Doesn't break the rest of the system.

2. **`pr` moves from `WaveRunSnapshot` to `WaveRun`.** A run *is* a snapshot of the wave. The separate `WaveRunSnapshot` struct is redundant, and mutating something called a "snapshot" is wrong. PR is a run output — it belongs on the run directly. Readers that access `run.snapshot.pr` update to `run.pr`.

3. **Retry count by walking `repair_of` chain, not a counter field.** No schema change. The chain *is* the count. Slightly more expensive (N queries for depth N, max 3), but N is tiny and correctness is obvious.

4. **Demo tests pure repair chain, not CI path.** A failing step exercises the full repair → backoff → escalation mechanism without requiring GitHub, webhooks, or PRs. The CI-fix path is a separate trigger that creates fresh runs — it doesn't exercise the `repair_of` chain. Test one thing well.

## Scope

- In scope:
  - `LF_HOME` env var in `lf_home_dir()`
  - PR state sync after `ops: land --create-pr`
  - Retry limit (3) with chain depth counting
  - `scripts/demo-algedonic.py` orchestration script
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
scripts/demo-algedonic.py

# Output shows:
# 1. Wave created
# 2. Run executed (step fails)
# 3. Repair run 1 dispatched (30s delay, repair_of links to failed run)
# 4. Repair run 2 dispatched (60s delay)
# 5. Repair run 3 dispatched (120s delay)
# 6. Repair run 3 fails → algedonic attention item created
# 7. Attention item visible via GET /attention
```

- `LF_HOME=/tmp/test cargo test` passes — lfd uses isolated state
- `cargo test repair_chain` — retry limit works at depth 3, backoff delays correct
- `cargo test pr_sync_after_land` — snapshot.pr updated after land
- `scripts/demo-algedonic.py` completes without errors
