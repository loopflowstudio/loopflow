# 01: FlowRun Container

**Finish line:** WaveRun is an iteration container (branch, worktree, PR). FlowRun is one flow execution within it. The executor operates on FlowRuns. Reactive triggers during active iterations create triggered FlowRuns, not new WaveRuns.

## Context

Signal cleanup shipped: `Signal` has only Repo/Wave/CiFailure, execution modes live on `wave.mode` (Loop/Cron/Manual), loop/cron tickers query waves by mode, manual starts dispatch directly with `trigger_id: None`. Incremental migration 022 (not hard reset) — appropriate since FlowRun wasn't included.

Trigger rename shipped: `Stimulus`/`stimuli` renamed to `Trigger`/`triggers` across Rust, Python, docs, and wave config. SQL migration 026 renames the `stimuli` table to `triggers` and updates FK columns. `ActivationSource` removed — trigger ID on the activation is sufficient. Cron promoted from trigger-level to wave-level (`wave.mode` + `wave.cron`).

Key decisions that carry forward:

- **Incremental migration path.** Signal cleanup used ALTER TABLE + table recreation (migration 022). Trigger rename used ALTER TABLE + DROP COLUMN (migration 026, requires SQLite 3.35.0+). FlowRun can follow the same pattern or consolidate into a hard reset if the migration count becomes unwieldy.
- **`ActivationEnvelope.trigger_id` is already `Option<LfdId>`.** No further activation-layer changes needed.
- **Cron `last_triggered_at` tracking lost.** The old code tracked this on the trigger. Currently cron waves attempt activation on every 30-second tick, relying on dedup to coalesce. Correct but noisy — add `last_cron_triggered_at` to the wave as part of this item.
- **No cron active-run check.** Unlike the loop ticker (which checks for active runs before dispatching), the cron ticker always dispatches. Address alongside FlowRun or accept the noise.
- **API field asymmetry.** Input APIs (`CreateWaveRequest`, `UpdateWaveRequest`) accept `flow`. Output (`WaveDto`) returns `primary_flow`. Intentional but the DTO layer should be consistent — decide whether to rename input to `primary_flow` or keep the asymmetry and document it.
- **Python API doesn't expose `mode`/`cron` yet.** The Python model deserializes them, but `create_wave`/`update_wave` don't accept them as parameters. Add when building FlowRun Python models.
- **Signal discriminant stability.** Removed Signal variants (Once, Loop, Cron) map to `Unspecified` via `from_i32`. Test `removed_discriminants_map_to_unspecified` guards this. New discriminant values must not reuse old slots.

## What to build

### Data model

**wave_runs** — strip execution fields (`flow`, `step_index`, `direction`, `area`, `flow_parents`, `activation_log_id`). Keep `repo`, `pr` directly. Keep `error`, `started_at`, `ended_at` for iteration lifecycle.

**flow_runs** — new table:

```sql
CREATE TABLE flow_runs (
    id TEXT PRIMARY KEY,
    wave_run_id TEXT NOT NULL REFERENCES wave_runs(id) ON DELETE CASCADE,
    flow TEXT NOT NULL,
    step_index INTEGER NOT NULL DEFAULT 0,
    direction TEXT NOT NULL DEFAULT '[]',
    area TEXT NOT NULL DEFAULT '[]',
    flow_parents TEXT NOT NULL DEFAULT '[]',
    activation_log_id TEXT,
    is_primary_flow INTEGER NOT NULL DEFAULT 0,
    status INTEGER NOT NULL,
    started_at TEXT,
    ended_at TEXT,
    error TEXT
);
```

**waves** — add `last_cron_triggered_at TEXT` (nullable).

### Rust types

- `FlowRun` struct with all fields from the table
- `FlowRunStatus` — reuse `WaveRunStatus` variants (Running, Completed, Failed, Stopped)
- Delete `WaveRunSnapshot`; move `repo` and `pr` to WaveRun top-level
- Strip `flow`, `step_index`, `direction`, `area`, `flow_parents`, `activation_log_id` from WaveRun
- Add `primary_flow_run: Option<FlowRun>` and `triggered_flows: Vec<FlowRun>` to WaveRun (in-memory, populated from DB)

### Store layer

- `map_flow_run_row`, `create_flow_run`, `update_flow_run`, `list_flow_runs(wave_run_id)`, `get_active_flow_run(wave_run_id)`
- `map_wave_run_row`: drop snapshot, read `repo`/`pr` directly, populate `primary_flow_run`/`triggered_flows` via join or separate query
- `create_wave_run`: also create initial FlowRun
- FlowRun trait methods + SharedStore delegation
- Both sqlite.rs and postgres.rs

### Executor refactor

**Riskiest part.** The executor loop changes from operating on `run.step_index` / `run.snapshot.flow` to:

1. Load the active FlowRun for this WaveRun
2. Step through the flow using `flow_run.step_index`
3. Between steps, check for pending triggered FlowRuns (`status = Pending`)
4. If found: complete current step, run triggered flow to completion, resume loop flow
5. When all FlowRuns complete, the WaveRun completes

Simple FIFO between steps. No mid-step interruption, no suspend/resume.

### Reactive trigger -> triggered flow

When Repo/CiFailure/Wave fires and a WaveRun is active:
1. Create a FlowRun with `is_primary_flow = 0`, `status = Pending`
2. The executor picks it up between steps of the primary_flow_run

When no WaveRun is active: drop the event. The wave picks up changes on next start (rebase happens naturally).

### DTO + Python

- `WaveRunDto`: add `primary_flow_run: Option<FlowRunDto>`, `triggered_flows: Vec<FlowRunDto>`
- Python models: FlowRun model, update WaveRun model
- Cron ticker: use `last_cron_triggered_at` to skip redundant activations

## Done when

```bash
cargo test --all
uv run pytest python/tests/
cargo clippy -- -D warnings
```

- WaveRun history shows FlowRuns (primary_flow_run + triggered_flows)
- Repo/CiFailure during an active run creates a triggered FlowRun on the same iteration
- Executor operates on FlowRun, not WaveRun fields
- `WaveRunSnapshot` deleted
- Cron ticker uses `last_cron_triggered_at` instead of always dispatching
