# 01: Signal Cleanup

**Finish line:** `Signal` enum has only reactive variants (Watch, Listen, Cron, CiFailure). Waves have `mode`, `flow`, and `loop_flow` fields. `WaveRun` is an iteration container with a `loop_flow_run` and `triggered_flows`. Loop ticker queries `wave.mode`, not Loop stimuli. Starting a wave dispatches directly — no manual stimulus. `lfq run` accepts `--flow`. All tests pass.

## Context

Default stimuli and the integrate flow shipped in the previous branch. The README already describes the target signal model — this sprint implements it.

## Core idea

Signals are reactive (external events that trigger flow overrides). Execution behavior lives on the wave itself:

```
Signal (reactive):     Watch | Listen | Cron | CiFailure
Wave fields:           mode (Loop|Once), flow, loop_flow
```

Starting a wave = running a flow. No "manual stimulus" concept. The wave's `flow` field is the default; callers can override at start time. The loop ticker uses `loop_flow` when re-triggering idle waves.

A WaveRun is an iteration — it owns the branch, worktree, and PR. FlowRuns execute within it. One iteration might run `ship-roadmap` as the primary flow, then `integrate` when main advances, then `ci-fix` when CI fails.

## Data model

### Wave

Add fields to the existing struct:

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| `mode` | `WaveMode` | `Loop` | Loop (re-trigger when idle) or Once (run once, stop) |
| `loop_flow` | `String` | `"ship-roadmap"` | Flow used by the loop ticker |

`wave.flow` already exists — it's the default for manual starts. `loop_flow` is what the ticker uses, typically a longer autonomous flow.

### WaveRun → iteration container

Strip execution details out of WaveRun. It becomes the iteration:

```rust
struct WaveRun {
    id: LfdId,
    wave_id: LfdId,
    iteration: u32,

    // workspace
    repo: String,
    branch: String,
    worktree: String,
    target_branch: String,

    // PR / stack
    pr: Option<PullRequest>,
    parent_run_id: Option<LfdId>,
    parent_pr_number: Option<u32>,
    stack_position: u32,
    stack_group_id: String,
    stack_status: WaveRunStackStatus,
    lineage_inferred: bool,

    // lifecycle
    status: WaveRunStatus,
    started_at: Option<OffsetDateTime>,
    ended_at: Option<OffsetDateTime>,

    // flow runs
    loop_flow_run: FlowRun,
    triggered_flows: Vec<FlowRun>,
}
```

### FlowRun (new)

```rust
struct FlowRun {
    id: LfdId,
    wave_run_id: LfdId,

    // what ran
    flow: String,
    step_index: u32,
    direction: Vec<String>,
    area: Vec<String>,
    flow_parents: Vec<String>,

    // trigger
    activation_log_id: Option<LfdId>,

    // lifecycle
    status: FlowRunStatus,  // reuse WaveRunStatus variants or define new
    started_at: Option<OffsetDateTime>,
    ended_at: Option<OffsetDateTime>,
    error: Option<String>,
}
```

### Delete WaveRunSnapshot

`WaveRunSnapshot` dissolves. `repo` and `pr` moved to WaveRun. `flow`, `direction`, `area` moved to FlowRun. Delete the struct.

### Signal enum

Remove `Once = 1` and `Loop = 2`. Keep discriminants stable (3, 4, 5, 6) for DB compatibility. `from_i32` returns `Unspecified` for legacy 1/2 rows.

### WaveMode enum (new)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum WaveMode {
    #[default]
    Loop,
    Once,
}
```

## DB migration

```sql
-- Wave fields
ALTER TABLE waves ADD COLUMN mode TEXT NOT NULL DEFAULT 'loop';
ALTER TABLE waves ADD COLUMN loop_flow TEXT NOT NULL DEFAULT 'ship-roadmap';

-- Delete legacy Once/Loop stimuli
DELETE FROM stimuli WHERE signal IN (1, 2);

-- FlowRun table
CREATE TABLE flow_runs (
    id TEXT PRIMARY KEY,
    wave_run_id TEXT NOT NULL REFERENCES wave_runs(id) ON DELETE CASCADE,
    flow TEXT NOT NULL,
    step_index INTEGER NOT NULL DEFAULT 0,
    direction TEXT NOT NULL DEFAULT '[]',
    area TEXT NOT NULL DEFAULT '[]',
    flow_parents TEXT NOT NULL DEFAULT '[]',
    activation_log_id TEXT,
    is_loop_flow INTEGER NOT NULL DEFAULT 0,
    status INTEGER NOT NULL,
    started_at TEXT,
    ended_at TEXT,
    error TEXT
);

CREATE INDEX idx_flow_runs_wave_run_id ON flow_runs(wave_run_id);

-- Migrate existing wave_runs data into flow_runs
INSERT INTO flow_runs (id, wave_run_id, flow, step_index, direction, area, flow_parents, activation_log_id, is_loop_flow, status, started_at, ended_at, error)
SELECT
    id || '-fr',
    id,
    flow,
    step_index,
    direction,
    area,
    flow_parents,
    activation_log_id,
    1,
    status,
    started_at,
    ended_at,
    error
FROM wave_runs;

-- Drop migrated columns from wave_runs
-- (SQLite doesn't support DROP COLUMN before 3.35; use table rebuild if needed)
```

The `is_loop_flow` column distinguishes the primary flow run from triggered flows. One flow run per wave_run has `is_loop_flow = 1`.

## Store layer

- New `FlowRun` CRUD: `create_flow_run`, `update_flow_run`, `list_flow_runs(wave_run_id)`
- `map_wave_row` reads `mode` and `loop_flow` columns in sqlite.rs and postgres.rs
- `InsertWave`/`UpdateWave` include `mode` and `loop_flow`
- New query: `list_loopable_waves()` — waves where `mode = 'loop' AND status != 'paused'`
- When loading a WaveRun, join flow_runs to populate `loop_flow_run` and `triggered_flows`
- When creating a WaveRun, also create the initial FlowRun

## Loop ticker

Replace `list_stimuli_by_signal(Signal::Loop)` with `list_loopable_waves()`. For each idle wave, dispatch an activation using `wave.loop_flow` as the flow. No stimulus ID needed.

## Starting a wave (waves.rs)

Delete `ensure_manual_stimulus`. `start_wave_run` dispatches an `ActivationEnvelope` directly:
- `stimulus_id`: `None`
- `source`: `ActivationSource::Manual`
- Flow comes from request body, falls back to `wave.flow`

Remove `"loop"` and `"once"` from `parse_stimulus`. Simplify `is_auto_stimulus` — all remaining signals are auto.

## Reactive stimulus → triggered flow

When a Watch/CiFailure/Listen stimulus fires during an active WaveRun, instead of creating a new WaveRun:
1. Create a FlowRun on the existing WaveRun with `is_loop_flow = 0`
2. Add it to `triggered_flows`
3. Execute it in the same worktree/branch

If no WaveRun is active, create a new WaveRun with the stimulus's flow as the `loop_flow_run`.

## ActivationEnvelope

Make `stimulus_id` an `Option<LfdId>`. Check all consumers:
- `PendingActivation.stimulus_id` → also `Option<LfdId>`
- `ActivationLog.stimulus_id` → also `Option<LfdId>`
- Dedup in `enqueue_pending_activation` — manual runs dedup by wave_id alone

## Executor

The executor currently tracks `step_index` on the WaveRun. Refactor to track it on the FlowRun instead. The executor loop becomes:
1. Load the active FlowRun for this WaveRun
2. Step through the flow
3. On completion, check for queued triggered_flows and execute them
4. When all FlowRuns complete, the WaveRun completes

`is_recurring` becomes `wave.mode == WaveMode::Loop || stimuli.iter().any(|s| matches!(s.signal, Signal::Watch | Signal::Cron))`.

## WaveConfig + DTO

- Add `mode: Option<String>` and `loop_flow: Option<String>` to `WaveConfig`
- Add `mode` and `loop_flow` to wave DTO in dto.rs
- Remove `Loop`/`Once` arms from `signal_str`
- WaveRun DTO includes nested FlowRun DTOs

## Python client + lfq

- `lfq run` accepts `--flow` flag (plumbing already exists in `client.run_wave(flow=...)`)
- Update `conftest.py` test fixture from `"kind": "loop"` to a valid signal
- Add `loop_flow` and `mode` to `create_wave` / `update_wave` in client.py and api.py

## Uncertainty

- **Triggered flows during no active run** — if a Watch fires and no WaveRun is active, we create a new WaveRun with `integrate` as the loop_flow_run. This is correct but slightly odd naming. The alternative is to always create a WaveRun first (via loop tick or manual start) and only then accept triggered flows. Simpler, but means Watch events are dropped when the wave is idle. Probably fine — an idle wave will pick up changes on next start anyway.
- **FlowRun execution order** — triggered flows queue behind the active loop_flow_run. If `integrate` fires mid-step, does it wait for the current step to finish? Probably yes — the executor finishes the current step, then runs the integrate flow, then resumes the loop flow. Need to design the interleaving.
- **SQLite column drop** — migration needs table rebuild for SQLite < 3.35. Check which version CI and production use.
- **loop_flow default** — `ship-roadmap` is the right default for autonomous waves. Confirm this flow exists as a builtin.
