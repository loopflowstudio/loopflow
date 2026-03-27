# Wave Crons — Multiple Scheduled Flows per Wave

## Problem

A wave has one flow and one mode. Maintenance rhythms (polish weekly, reduce monthly, governance daily) require either fragmenting a wave's identity across `my-wave`, `my-wave-polish`, `my-wave-reduce` — each duplicating area, direction, and README — or bolting multiple scheduled flows onto the wave itself.

The wave *is* the scope. Crons are maintenance rhythms layered on top. A member wave should grind build with workers *and* polish weekly without needing a second wave. A root wave should run governance crons without pretending it has workers.

## Approach

Wave config gains `crons`: a list of flow + schedule pairs. Cron runs are independent of the worker pool — they don't count against `workers` capacity, get their own ephemeral worktrees, and fire regardless of whether workers are busy.

`mode: cron` is removed. All scheduled work uses `crons`. What was `mode: cron` becomes `workers: 0` + a cron entry.

```yaml
# member wave — workers grind build, crons sweep maintenance
flow: build
workers: 2
mode: loop
crons:
  - flow: wave-polish
    schedule: "0 0 * * 1"      # weekly Monday
  - flow: wave-reduce
    schedule: "0 0 1 * *"      # monthly 1st

# root wave — no workers, all work on crons
flow: garden
workers: 0
mode: manual
crons:
  - flow: govern-identity
    schedule: "0 0 * * 0"
  - flow: govern-coordination
    schedule: "0 0 * * *"
  - flow: integrate
    schedule: "0 */6 * * *"
```

`flow` + `workers` + `mode` = the primary work. What the wave *does*.
`crons` = flows that fire on schedule. Each runs once when triggered, not in the worker pool.
`mode` is now `loop | manual` only — `cron` is gone.

### Database

New `wave_crons` table, following the same pattern as `triggers` (separate table with per-entry state, not JSON in the waves column):

```sql
CREATE TABLE wave_crons (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    flow TEXT NOT NULL,
    schedule TEXT NOT NULL,
    last_triggered_at BIGINT,
    created_at BIGINT NOT NULL
);
CREATE INDEX idx_wave_crons_wave ON wave_crons(wave_id);
```

`last_triggered_at` is per-cron-entry, not per-wave. The current cron poller uses `list_activation_log(wave_id, 1)` to approximate last-triggered, which collapses all crons for a wave into one timestamp. Per-entry tracking lets independent schedules fire independently.

### Data model

Rust:
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct WaveCron {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub flow: String,
    pub schedule: String,
    pub last_triggered_at: Option<i64>,
    pub created_at: i64,
}
```

Python:
```python
class WaveCron(BaseModel):
    id: str
    wave_id: str
    flow: str
    schedule: str
    last_triggered_at: Optional[int] = None
    created_at: Optional[int] = None
```

Swift (LoopflowCore):
```swift
public struct WaveCron: Codable, Sendable, Equatable, Identifiable {
    public let id: String
    public let waveId: String
    public let flow: String
    public let schedule: String
    public let lastTriggeredAt: Date?
    public let createdAt: Date?
}
```

Wave gains `crons` in all three models:
- Rust: `pub crons: Vec<WaveCron>` (populated by store queries, not a DB column)
- Python: `crons: list[WaveCron] = Field(default_factory=list)`
- Swift: `public var crons: [WaveCron]`

### Config parsing

`WaveConfig` gains:
```rust
pub crons: Option<Vec<WaveCronDef>>,

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WaveCronDef {
    pub flow: String,
    pub schedule: String,
}
```

During `create_wave_handler`, cron defs are converted to `WaveCron` rows in the `wave_crons` table — same pattern as triggers (YAML config → DB rows, not stored as JSON).

### Workers: 0

Allow `workers: 0` for waves that are purely cron-driven. Remove the `.max(1)` enforcement in `create_wave_workers` and `wave.workers()`. A wave with `workers: 0` and no crons is valid (paused/manual) but the primary flow never auto-dispatches.

### Dispatch

The cron poller (`spawn_cron_poller`) replaces the existing `mode: cron` path entirely:

1. Query all `wave_crons` entries joined with waves (skip paused waves).
2. For each entry, call `should_activate_cron(entry.schedule, entry.last_triggered_at)`.
3. On fire:
   - Create a `WaveRun` with `flow = entry.flow`, inheriting the wave's area and direction
   - Use `spawn_immediate_activation` (parallel path) — always gets its own ephemeral worktree
   - Update `entry.last_triggered_at` in the store
   - Log to `activation_log` with reason `"cron [{flow}] schedule {schedule} due"`

The old `list_cron_waves` / `wave.cron` path is removed. `should_activate_cron` stays — it now reads `last_triggered_at` from the cron entry instead of from `activation_log`.

Cron runs bypass per-wave `workers` capacity but still acquire a global `Scheduler` semaphore slot. If the global ceiling is hit, the cron activation queues normally.

### Migration

Migration converts existing `mode: cron` waves:

```sql
-- For each wave where mode = 'cron' and cron IS NOT NULL:
-- 1. Insert a wave_crons row with flow = primary_flow, schedule = cron
-- 2. Set mode = 'manual', workers = 0
-- 3. The cron column is dropped from waves
INSERT INTO wave_crons (id, wave_id, flow, schedule, created_at)
SELECT lfd_id(), id, primary_flow, cron, strftime('%s', 'now')
FROM waves WHERE mode = 'cron' AND cron IS NOT NULL;

UPDATE waves SET mode = 'manual', workers = 0 WHERE mode = 'cron';

-- Then drop the cron column (via table rebuild in SQLite)
```

`WaveMode` loses the `Cron` variant. `Wave.cron` field is removed. `WaveConfig.cron` is removed. `list_cron_waves` store method is removed.

### Store queries

```sql
-- ListWaveCrons: all crons for a wave
SELECT id, wave_id, flow, schedule, last_triggered_at, created_at
FROM wave_crons WHERE wave_id = ?1 ORDER BY created_at;

-- ListAllActiveCrons: for the poller (skip paused waves)
SELECT wc.id, wc.wave_id, wc.flow, wc.schedule, wc.last_triggered_at, wc.created_at
FROM wave_crons wc
JOIN waves w ON wc.wave_id = w.id
WHERE w.status != 4
ORDER BY wc.wave_id, wc.created_at;

-- UpdateCronLastTriggered
UPDATE wave_crons SET last_triggered_at = ?2 WHERE id = ?1;

-- DeleteWaveCrons: cleanup on wave delete (CASCADE handles this too)
DELETE FROM wave_crons WHERE wave_id = ?1;
```

### HTTP API

- `GET /waves/{id}` response includes `crons: [...]`
- `POST /waves` accepts `crons` in the body (or reads from YAML config)
- `PUT /waves/{id}` can update crons (replace-all semantics)
- `GET /waves/{id}/crons` — dedicated endpoint for cron list with last-triggered times

### Concerto UI

Wave detail view shows crons as a section below triggers:

```
Crons
  wave-polish    every Monday     last: 2h ago
  wave-reduce    1st of month     last: 3d ago
```

Human-readable schedule descriptions (croner or manual mapping for common patterns). Each entry shows last-triggered relative time.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Separate waves per flow | Each gets its own backlog and identity | Fragments scope — area/direction/README duplicated, harder to reason about "what does this wave do" |
| JSON column on waves table | Simpler schema, no joins | Crons need mutable per-entry state (last_triggered_at); JSON update is awkward and loses query indexing |
| Extend existing triggers table | Reuse existing signal/trigger machinery | Triggers respond to events (repo change, CI failure); crons are time-based. Different dispatch semantics. Overloading `signal` with a cron type conflates two concepts |
| Keep `mode: cron` alongside `crons` | Backwards compat, no migration | Two mechanisms for the same thing. Config footgun. Extra code path in the poller for no benefit |

## Key decisions

**Separate table, not JSON.** Crons have mutable per-entry state (`last_triggered_at`). A separate table gives clean queries, proper indexing, and CASCADE deletes. Follows the triggers pattern.

**Cron runs bypass `workers` but respect global slots.** A wave's `workers` budget is for the primary flow. Crons are supplementary — if they competed with workers, a busy wave would starve its own maintenance. The global `Scheduler` semaphore prevents runaway concurrency across the system.

**Per-entry last-triggered, not per-wave.** The current cron poller approximates last-triggered from activation_log (most recent entry for the wave). With multiple crons per wave, this collapses independent schedules. A weekly polish and daily governance would share one timestamp. Per-entry tracking is the only correct approach.

**`mode: cron` is gone.** One mechanism for scheduled work: `crons`. Existing `mode: cron` waves are migrated to `workers: 0` + a cron entry. `WaveMode` becomes `Loop | Manual`.

**`workers: 0` is valid.** Root waves that are purely cron-driven shouldn't need a dummy worker. Remove the `.max(1)` enforcement entirely.

## Scope

- **In scope:** `wave_crons` table + migration, `WaveCron` type in Rust/Python/Swift, config parsing, cron poller rewrite (single path), `workers: 0` support, remove `mode: cron` + `wave.cron`, HTTP API, store queries, activation logging
- **Out of scope:** Concerto UI for cron management (read-only display only), cron expression validation beyond what the `cron` crate provides, retry/repair logic for failed cron runs (use existing repair mechanism)

## Done when

- `crons` field on wave config YAML parsed and stored in `wave_crons` table
- `WaveCron` type exists in Rust, Python, and Swift models
- `lfd` cron poller fires supplementary flows on schedule independently of worker pool
- Each cron entry tracks its own `last_triggered_at`
- `workers: 0` works for waves with crons
- Root wave with `workers: 0` + governance crons dispatches correctly
- Member wave with `workers: 2` + polish/reduce crons dispatches both primary and supplementary flows
- `cargo test` covers: cron scheduling logic, `workers: 0` acceptance, cron activation logging
- `GET /waves/{id}` returns crons in the response
