# Worker Pools

## Problem

Waves have a binary `serialized: bool` — either one run at a time or unlimited concurrent runs. VSM's S3 (control) needs to tune concurrency per wave: "this wave can handle 3 parallel runs." Any wave with trigger-driven work benefits from bounded concurrency without being forced into single-threading.

The current default (`serialized: false`) means unlimited parallelism, which is uncontrolled. The safer default is bounded — every wave should have a finite worker count.

## Approach

Replace `serialized: bool` with `workers: u32` on the `Wave` type and in wave YAML config. Default `workers: 1`. All dispatch paths count active runs against the limit.

### Data model

```rust
// Wave struct — replace serialized: bool
pub workers: u32,  // default 1, minimum 1
```

```yaml
# wave/<name>/<name>.yaml
flow: build-or-silent
workers: 3
```

Add `workers` to `WaveConfig` YAML parsing. Add `workers` to `WaveDto` HTTP response (replace `serialized: bool`). Add `workers` to Python `Wave` model. Add `workers` column to database, migration from `serialized`.

### Dispatch logic

The binary routing (`if wave.serialized { enqueue } else { spawn_immediate }`) becomes a capacity check. All triggers use the same path:

1. Count active runs for this wave (Pending + Running + Waiting status)
2. If count < workers: spawn via `create_parallel_wave_run()` (per-run worktree)
3. If count >= workers: enqueue as pending activation

`dispatch_pending_activations()` does the same check — poll pending queue, spawn if under capacity. This replaces the current "check for any active run" gate.

Special case for `workers: 1`: use `create_wave_run_with_id()` (shared worktree) to preserve the current serialized behavior where runs reuse the wave's worktree. This keeps branch continuity for serial waves.

### New store query

Add `CountActiveWaveRuns` query:

```sql
SELECT COUNT(*) FROM wave_runs
WHERE wave_id = ?1 AND status IN (?2, ?3, ?4)
```

Replace `get_active_wave_run()` (returns Option<WaveRun>) with `count_active_wave_runs()` (returns u32) in the dispatch paths. Keep `get_active_wave_run()` for the wave-completion idle-check (needs to know if *any* run exists).

### Trigger routing changes

All four trigger sites (cron, watch, loop_ticker, wave-completion listener) currently branch on `wave.serialized`. Replace with:

```rust
let active = store.count_active_wave_runs(wave.id()).await?;
if active < wave.workers {
    spawn_immediate_activation(...)
} else {
    enqueue_pending_activation(...)
}
```

`spawn_immediate_activation()` already falls back to enqueue when the scheduler is at capacity — that stays. The worker-pool check is an additional gate before even trying.

### Migration

Database migration `019_wave_workers.sql`:

```sql
ALTER TABLE waves ADD COLUMN workers INTEGER NOT NULL DEFAULT 1;
UPDATE waves SET workers = 1;  -- all existing waves get workers=1
```

Keep reading `serialized` from YAML for backwards compat during a transition period. `serialized: true` maps to `workers: 1`. `serialized: false` or absent also maps to `workers: 1` (new default — no unlimited mode).

The `serialized` column stays in the DB for one release cycle, then gets dropped. The Rust field gets `#[deprecated]` and is ignored in favor of `workers`.

### HTTP API

`CreateWaveRequest` and `UpdateWaveRequest` accept `workers: Option<u32>`. Still accept `serialized: bool` for backwards compat (maps to workers=1). `WaveDto` exposes `workers: u32` and drops `serialized: bool`.

### Python model

Add `workers: int = 1` to `Wave` model. Remove `serialized` if it exists (it doesn't currently).

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep `serialized` + add `max_concurrent` | Two fields for one concept | Confusing — which takes precedence? |
| `workers: 0` means unlimited | Matches common convention | Every wave should have a finite limit. Unlimited is a footgun for agent systems. |
| Workers as a runtime-only setting (not in YAML) | Less config surface | Wave YAML is the source of truth for wave behavior. Workers is a core property. |
| Pool as a separate object | More flexible | Over-engineering. A wave *is* a pool of work. |

## Key decisions

**Default is 1, not "unlimited."** The old default (`serialized: false`) gave unlimited parallelism. The new default is 1 worker. This is a behavior change for non-serialized waves that never set `serialized: true`. Existing waves that relied on unlimited parallel runs will now serialize. This is intentional — unbounded concurrency is the wrong default for agent systems.

**Per-run worktrees for workers > 1.** When `workers > 1`, each run gets its own worktree (via `create_parallel_wave_run()`). When `workers == 1`, runs reuse the shared wave worktree (via `create_wave_run_with_id()`). This preserves the existing optimization for serial waves while supporting safe concurrency.

**Count-based gating, not slot reservation.** Dispatch counts active runs and compares to the limit. No reservation or lock-ahead. If two triggers fire simultaneously, both might pass the count check and spawn, briefly exceeding the limit by 1. This is acceptable — the scheduler's global capacity limit provides a hard backstop, and the slight overshoot resolves naturally when one run completes.

**Workers composes with all modes.** `loop` + `workers: 3` means 3 persistent loopers. `cron` + `workers: 3` means on schedule, launch up to 3. `flow` triggers respect the limit. No special-casing per mode.

## Scope

- In scope: `workers` field on Wave, YAML parsing, dispatch changes, DB migration, HTTP API, Python model, backwards compat for `serialized`
- Out of scope: dynamic worker pool resizing at runtime (S3 governance can update `workers` in YAML, but that takes effect on next activation), per-step concurrency limits, worker affinity/routing

## Done when

- `workers: N` in wave YAML controls concurrent run capacity
- Default is `workers: 1` (no unlimited mode)
- Dispatch respects the limit: excess activations queue
- `serialized: true` in YAML still works (maps to `workers: 1`)
- A wave with `workers: 3` runs up to 3 concurrent activations
- `cargo test --all` passes
- Existing tests adapted (no `serialized` references in new code paths)

Advancing chord-model goals:
> "Governance flows as the chord's reusable VSM lens (s5/s4/s3/s2 as separate flows)"

Worker pools give S3 (control) a concrete lever: adjust `workers` based on observed wave health and capacity.
