# 01: Signal Cleanup

**Finish line:** `Signal` has only Watch, Listen, CiFailure. Execution modes live on `wave.mode`. WaveRun contains FlowRuns. Starting a wave dispatches directly — no manual stimulus.

## Problem

Signal enum conflates execution modes (Loop, Once, Cron) with reactive triggers (Watch, Listen, CiFailure). Starting a wave creates a "manual stimulus" — an oxymoron. The loop ticker queries stimuli when it should query wave state. A daily release wave has no clean home — Cron is a stimulus but it's really an execution mode. WaveRun carries execution details (flow, step_index, direction, area) that belong on a per-flow-execution container, making it impossible to run multiple flows within one iteration.

The README already describes the target model. This sprint implements it.

## Approach

Split the Signal enum. Move execution modes to `wave.mode`. Introduce FlowRun as the unit of flow execution within a WaveRun iteration. Delete the "manual stimulus" concept — starting a wave dispatches directly.

```
Before:
  Signal: Unspecified | Once | Loop | Watch | Cron | Listen | CiFailure
  WaveRun: carries flow, step_index, direction, area directly
  Manual start: create Once stimulus → create ActivationEnvelope → dispatch

After:
  Signal: Unspecified | Watch | Listen | CiFailure
  WaveMode: Loop | Cron | Manual (on Wave struct)
  Wave: cron expression on wave.cron (nullable, required when mode=Cron)
  WaveRun: iteration container (branch, worktree, PR)
  FlowRun: flow execution (flow, step_index, direction, area)
  Manual start: create ActivationEnvelope directly → dispatch
```

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Signal cleanup only, no FlowRun | ~600 LOC, simpler | Half-feature. Can't run triggered flows within an iteration. Leaves WaveRun carrying execution details it shouldn't. |
| FlowRun with complex interleaving (suspend/resume mid-step) | More responsive to reactive triggers | Over-engineering. Current agents can't be suspended mid-step anyway. Simple FIFO between steps is correct. |
| Reactive stimuli create new WaveRuns during active runs | No executor changes needed | Creates parallel worktrees/branches for what should be same-iteration work. `integrate` should run in the same worktree, not a new one. |

## Key decisions

**One PR, not two.** ~1500 LOC, over the ~1000 target but acceptable. Signal cleanup without FlowRun is a half-feature. FlowRun without signal cleanup leaves the confusing stimulus model. Ship the complete concept.

**Drop reactive events when no WaveRun is active.** If Watch fires on an idle wave, the event is lost. The wave picks up changes on next start (rebase happens naturally). This avoids creating WaveRuns with nonsensical "integrate as primary flow" and keeps the model clean. An idle wave isn't working — no point running integrate on nothing.

**Simple triggered flow execution.** Between steps of the loop_flow_run, the executor checks for pending triggered FlowRuns. If one exists, it runs the entire triggered flow to completion, then resumes the loop flow. No mid-step interruption, no complex suspend/resume. FIFO order.

**Hard reset migration.** No users yet. Consolidate all migrations into a single `001_baseline.sql` with the full target schema. Delete the 22 existing migration files. Existing local DBs get wiped.

**`ship-roadmap` as loop_flow default.** Confirmed as builtin flow (`flows/code/ship-roadmap.yaml`). Correct default for autonomous waves.

**Manual dedup by wave_id.** `ActivationEnvelope.stimulus_id` becomes `Option<LfdId>`. `get_pending_for_stimulus` gains a second path: when stimulus_id is None (manual/loop-ticker starts), coalesce by wave_id alone. When present (reactive stimuli), coalesce by (wave_id, stimulus_id) as today.

## Scope

**In scope:**
- Remove `Once`/`Loop`/`Cron` from Signal enum (keep discriminants 3, 5, 6 stable for Watch, Listen, CiFailure)
- Add `WaveMode` enum (Loop, Cron, Manual) and `wave.mode`, `wave.loop_flow`, `wave.cron` fields
- FlowRun struct, table, CRUD
- WaveRun → iteration container (strip snapshot, carry repo/pr directly)
- Delete `WaveRunSnapshot`
- Loop ticker: query `wave.mode == Loop` instead of Loop stimuli
- Manual start: delete `ensure_manual_stimulus`, dispatch directly
- `ActivationEnvelope.stimulus_id` → `Option<LfdId>` (+ PendingActivation, ActivationLog)
- Executor: operate on FlowRun, check triggered flows between steps
- `parse_stimulus`: remove "loop"/"once"/"cron"
- Cron ticker: query `wave.mode == Cron` instead of Cron stimuli (same pattern as loop ticker)
- WaveConfig: add `mode`, `loop_flow`, `cron`
- DTO: add `mode`, `loop_flow`, `cron` to wave; FlowRun DTO on WaveRun
- Python: update conftest.py fixture, add `mode`/`loop_flow`/`cron` to models

**Out of scope:**
- Triggered flow interleaving within a step (agents can't be suspended)
- Reactive stimulus area-based filtering (the step handles via early exit)
- Renaming `stimulus.kind` → `stimulus.signal` in external API (deferred, lockstep change)
- Timestamp-based migration versioning (follow-up)

## Implementation sequence

### 1. Data model — hard reset migration

No users yet. Instead of careful ALTER TABLE / data migration, hard-reset the schema. Drop and recreate affected tables in the initial migration, or consolidate into the base schema. Existing local DBs get wiped.

The target schema for new/changed tables:

**waves** — add columns:
- `mode TEXT NOT NULL DEFAULT 'loop'` — Loop | Cron | Manual
- `loop_flow TEXT NOT NULL DEFAULT 'ship-roadmap'` — flow for loop ticker
- `cron TEXT` — cron expression, required when mode = 'cron'

**stimuli** — no Once/Loop/Cron rows. Signal values: Watch (3), Listen (5), CiFailure (6).

**pending_activations** / **activation_log** — `stimulus_id` becomes nullable (no FK constraint needed).

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
    is_loop_flow INTEGER NOT NULL DEFAULT 0,
    status INTEGER NOT NULL,
    started_at TEXT,
    ended_at TEXT,
    error TEXT
);

CREATE INDEX idx_flow_runs_wave_run_id ON flow_runs(wave_run_id);
```

Note: `started_at`, `ended_at`, and `error` remain on both WaveRun (iteration lifecycle) and FlowRun (flow lifecycle). WaveRun.status reflects the overall iteration; FlowRun.status reflects individual flow execution.

### 2. Rust types

- `WaveMode` enum (Loop, Cron, Manual) with serde, Default, as_str, from_str
- Add `mode: WaveMode`, `loop_flow: String`, and `cron: Option<String>` to `Wave`
- `FlowRun` struct with all fields
- `FlowRunStatus` — reuse `WaveRunStatus` variants (Running, Completed, Failed, Stopped)
- Remove `Once = 1`, `Loop = 2`, and `Cron = 4` from `Signal`; `from_i32(1|2|4)` → `Unspecified`
- Delete `WaveRunSnapshot`; move `repo` and `pr` to WaveRun top-level
- Strip `flow`, `step_index`, `direction`, `area`, `flow_parents`, `activation_log_id` from WaveRun
- Add `loop_flow_run: Option<FlowRun>` and `triggered_flows: Vec<FlowRun>` to WaveRun (in-memory only, populated from DB)
- `ActivationEnvelope.stimulus_id` → `Option<LfdId>`
- `PendingActivation.stimulus_id` → `Option<LfdId>`
- `ActivationLog.stimulus_id` → `Option<LfdId>`

### 3. Store layer

- `map_wave_row`: read `mode`, `loop_flow`, and `cron` columns
- Wave insert/update: include `mode`, `loop_flow`, and `cron`
- `list_loopable_waves()`: `SELECT * FROM waves WHERE mode = 'loop' AND status != 2` (not paused)
- `list_cron_waves()`: `SELECT * FROM waves WHERE mode = 'cron' AND status != 2`
- `map_flow_run_row`, `create_flow_run`, `update_flow_run`, `list_flow_runs(wave_run_id)`, `get_active_flow_run(wave_run_id)`
- `map_wave_run_row`: drop snapshot, read `repo`/`pr` directly, populate `loop_flow_run`/`triggered_flows` via join or separate query
- `create_wave_run`: also create initial FlowRun
- `get_pending_for_stimulus`: handle `stimulus_id = None` case — match on wave_id with `stimulus_id IS NULL`
- Add FlowRun trait methods to store trait + SharedStore delegation
- Both sqlite.rs and postgres.rs

### 4. Loop ticker

Replace:
```rust
store.list_stimuli_by_signal(Signal::Loop.as_i32())
```
With:
```rust
store.list_loopable_waves()
```

For each idle wave, dispatch with `stimulus_id: None`, `source: ActivationSource::Poll`, and `wave.loop_flow` as the flow override.

### 4b. Cron ticker

Same pattern as loop ticker. Replace querying `Signal::Cron` stimuli with:
```rust
store.list_cron_waves()  // WHERE mode = 'cron' AND status != 2
```

For each wave whose `wave.cron` expression matches the current time, dispatch with `stimulus_id: None`, `source: ActivationSource::Poll`, and `wave.flow` as the flow.

### 5. Manual start (waves.rs)

Delete `ensure_manual_stimulus`. In `start_wave_run`:
```rust
let envelope = ActivationEnvelope::new(
    wave.id(),
    None,  // no stimulus
    ActivationSource::Manual,
    "manual run requested via API",
    "", "", "main",
);
```

Flow comes from request body (`--flow` flag), falls back to `wave.flow`.

Remove `"loop"`, `"once"`, and `"cron"` from `parse_stimulus`. Delete `is_auto_stimulus` — all remaining signals are reactive (auto by definition).

### 6. Executor refactor

**Riskiest step.** This and step 7 change how the core execution loop works. The FlowRun interleaving logic (check pending between steps, run to completion, resume) needs careful testing.

The executor loop changes from operating on `run.step_index` / `run.snapshot.flow` to:

1. Load the active FlowRun for this WaveRun
2. Step through the flow using `flow_run.step_index`
3. Between steps, check for pending triggered FlowRuns (`status = Pending`)
4. If found: complete current step, run triggered flow to completion, resume loop flow
5. When all FlowRuns complete, the WaveRun completes

`is_recurring` becomes `wave.mode == WaveMode::Loop || wave.mode == WaveMode::Cron`.

### 7. Reactive stimulus → triggered flow

When Watch/CiFailure/Listen fires and a WaveRun is active:
1. Create a FlowRun with `is_loop_flow = 0`, `status = Pending`
2. The executor picks it up between steps of the loop_flow_run

When no WaveRun is active: drop the event. The wave picks up changes on next start.

### 8. WaveConfig + DTO + Python

- `WaveConfig`: add `mode: Option<String>`, `loop_flow: Option<String>`, `cron: Option<String>`
- `WaveDto`: add `mode: String`, `loop_flow: String`, `cron: Option<String>`
- `WaveRunDto`: add `loop_flow_run: Option<FlowRunDto>`, `triggered_flows: Vec<FlowRunDto>`
- `StimulusDto.kind`: remove `signal_str` arms for Loop/Once/Cron
- Python `Wave` model: add `mode: str`, `loop_flow: str`, `cron: Optional[str]`
- Python conftest: change `"kind": "loop"` to `"kind": "watch"` in WAVE_FULL fixture
- Python `create_wave`/`update_wave`: accept `mode`, `loop_flow`, `cron` params

## Done when

```bash
cargo test --all                       # All Rust tests pass
uv run pytest python/tests/            # All Python tests pass
cargo clippy -- -D warnings            # No warnings
```

Observable outcomes:
- `Signal` only has Watch, Listen, CiFailure (no Once/Loop/Cron)
- `WaveMode` is Loop, Cron, or Manual
- Creating a wave sets `mode = 'loop'` by default
- Loop and cron tickers query waves by mode, not stimuli
- `lfq run <wave>` works without creating a Once stimulus
- `lfq run <wave> --flow build` overrides the wave's default flow
- A cron wave (`mode: cron, cron: "0 9 * * *"`) runs on schedule
- WaveRun history shows FlowRuns (loop_flow_run + triggered_flows)
- Watch/CiFailure during an active run creates a triggered FlowRun on the same iteration
- Single `001_baseline.sql` migration; old migration files deleted

## Wave alignment

**Goals advanced:**
- "Signal cleanup: execution modes (Loop/Cron/Manual) separated from reactive triggers (Watch/Listen/CiFailure)" — this is the core deliverable
- "WaveRun/FlowRun split: WaveRun is an iteration (branch, worktree, PR); FlowRun is one flow execution within it"
- "Starting a wave = running a flow. No manual stimulus. `lfq run` accepts `--flow`."

**Risks checked:**
- "CI recursion guard coupling" — guard moves from `snapshot.flow` to `flow_run.flow`. Same logic, new location. Verified in implementation.
- "Integrate-upstream false positives" — unchanged by this work. The step handles via early exit.

**Migration risk eliminated.** Hard reset to `001_baseline.sql` — no data migration, no partial-apply risk. Existing local DBs get wiped.
