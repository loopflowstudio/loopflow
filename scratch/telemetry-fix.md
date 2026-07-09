# Telemetry pass: make `run_events` a reliable API

Implement in one pass. The dashboard (`dashboard-design.md`) is the consumer;
the audit that motivates each change is `telemetry.md`. Run `lf doctor` before
you start — it exits 1 today, and the point of this work is that it exits 0.

## What is broken

Measured on the real ledger, not inferred:

1. `run_id` names a **tree of processes** (a nested `lf` inherits `LF_RUN_ID`).
   134 run_ids carry >1 command; one carries 9. No parent link exists.
2. **2176/2176 terminal run rows name no work.** `command` lives only on
   `run/started`, so cost cannot be attributed without an ambiguous join.
3. `node='step'` (309 rows) and `node='skill'` (148) are the same concept, split
   by migration 054 renaming the column and not the values.
4. `repo` is `main_repo.file_name()`, so 888/898 values are temp roots.
5. `model` is absent; `provider` exists only from 2026-07-09.

Already fixed, do not redo: cost now accumulates (every usage field is
cumulative-to-that-point), and the first ledger write failure per process warns.

## Clean cutover: the old rows go

`run_events` is truncated by this migration. We are not carrying 5247 rows of
history that cannot answer a question: their cost is undercounted (the overwrite
bug), 888/898 of their `repo` values are temp roots, their node vocabulary is
split, and not one of them can be attributed to a process. Keeping them would
force a nullable `process_id`, a legacy branch in every reader, and a `lf doctor`
that can never go green — and a monitor that is permanently red is a monitor
people learn to ignore. That is precisely how a 29-hour outage went unnoticed.

`process_id` is therefore `NOT NULL`. There is no legacy path anywhere.

The per-repo file journals (`.lf/journal/runs/*/events.jsonl`) are untouched and
remain the durable per-repo record. Back up first:
`cp ~/.lf/lfd.db ~/.lf/lfd.db.bak-pre057`.

## Non-goals

- The PR delivery record (run/wave → PR → merge). Separate pass; it blocks the
  dashboard's PR metrics, not its run metrics.
- Emitting `escalated` (currently 0 rows in 5247). Separate pass.
- Backfilling. Nothing is migrated forward; the table starts empty.

## The contract

A row is one event in one process.

- `run_id` — the **trace**. Stable across nested `lf`. Unchanged.
- `process_id` — the **span**. Minted once per process. A process's `started`
  and terminal rows share it.
- `parent_process_id` — the calling process's span, or NULL at the root.
- A **terminal row is self-describing**: it carries the same `command` its
  `started` row carried. A reader never joins to learn what a cost bought.
- `node ∈ {run, flow, skill}`, `event ∈ {started, completed, errored, escalated}`,
  enforced by CHECK.
- `repo` is the **absolute main-repo root**, not a basename.
- Every usage field (`input_tokens`, `output_tokens`, `cache_read_tokens`,
  `cost_usd`, `duration_secs`) is **cumulative to that point in the process**.
  A span's own figure is the diff against the previous boundary row in the same
  `process_id`. The terminal row is the process total.

## 1. Migration `057_run_events_identity.sql`

Register in `migrations.rs` `ALL_MIGRATIONS` after `056_run_events_provider`.
SQLite cannot add a CHECK with `ALTER`, so rebuild the table.

```sql
-- The run ledger gets span identity. Pre-contract rows are dropped rather than
-- carried: their cost is undercounted, their repo is a basename, their node
-- vocabulary is split, and none of them can be attributed to a process. See
-- telemetry.md. The per-repo file journals remain the durable record.
--
-- Still no primary key: several writers share a run_id (a child `lf` inherits
-- LF_RUN_ID) — that is the trace. process_id is the span, one per process.
DROP TABLE IF EXISTS run_events;

CREATE TABLE run_events (
    run_id TEXT NOT NULL,
    process_id TEXT NOT NULL,
    parent_process_id TEXT,
    seq BIGINT NOT NULL,
    ts BIGINT NOT NULL,
    repo TEXT,
    worktree TEXT,
    wave TEXT,
    node TEXT NOT NULL CHECK (node IN ('run', 'flow', 'skill')),
    event TEXT NOT NULL CHECK (event IN ('started', 'completed', 'errored', 'escalated')),
    command TEXT,
    flow TEXT,
    skill TEXT,
    step_index BIGINT,
    error TEXT,
    input_tokens BIGINT,
    output_tokens BIGINT,
    cache_read_tokens BIGINT,
    cost_usd REAL,
    duration_secs REAL,
    provider TEXT,
    model TEXT,
    context TEXT
);

CREATE INDEX idx_run_events_ts ON run_events(ts);
CREATE INDEX idx_run_events_run ON run_events(run_id);
CREATE INDEX idx_run_events_process ON run_events(process_id);
```

Do **not** add `057` to `RENAME_CONVERGENCE_MIGRATIONS` — it must not silently
skip. It runs once, keyed by version.

## 2. Writer — `rust/loopflow/src/journal/mod.rs`

Add `pub const LF_PROCESS_ID_ENV: &str = "LF_PROCESS_ID";`

`RunContext` gains three fields:

```rust
process_id: LfdId,
parent_process_id: Option<LfdId>,
/// The argv this process was invoked with, captured at `run/started` so the
/// terminal row can name its own work without a join.
command: Option<String>,   // serialized argv JSON, as today
```

In `ensure_run_context`:

- `run_id` logic is unchanged (inherit `LF_RUN_ID` or mint + export).
- `parent_process_id` = `std::env::var(LF_PROCESS_ID_ENV).ok().map(LfdId::from_raw)`.
  Read it **before** overwriting.
- `process_id` = `LfdId::default()`, always minted, never inherited.
  Then `std::env::set_var(LF_PROCESS_ID_ENV, process_id.as_str())` so children
  see this process as their parent.
- `command` = `fields.command.as_ref().and_then(|argv| serde_json::to_string(argv).ok())`.
- `repo` = the **absolute path** of `main_repo.as_deref().unwrap_or(repo_root)`,
  via `.display().to_string()`. Delete the `.file_name()` call.

In `ledger_insert`, the row gains:

- `process_id: context.process_id.as_str().to_string()`
- `parent_process_id: context.parent_process_id.as_ref().map(|id| id.as_str().to_string())`
- `command`: the event's own argv, falling back to `context.command.clone()` —
  so terminal rows inherit the started row's argv and name their own work.
- `model: snapshot_model()` (see below).

One subtlety worth stating, because it is easy to get backwards: a nested `lf`
**inherits `LF_RUN_ID` but never inherits `LF_PROCESS_ID`**. It reads the parent's
value as its `parent_process_id`, then overwrites the variable with its own id.
`std::env::set_var` affects this process and its future children only, so the
parent's own environment is untouched.

Model: `PendingUsage` is `Copy`, so do not put a `String` in it. Add a sibling
thread-local:

```rust
thread_local! { static PENDING_MODEL: RefCell<Option<String>> = const { RefCell::new(None) }; }
pub fn record_model(model: &str) { … }
fn snapshot_model() -> Option<String> { … }
```

Clear it wherever `clear_usage()` is called.

## 3. Writer — `rust/loopflow/src/engine/agent.rs`

At the existing launch site (~line 918, beside `record_provider`):

```rust
let (harness, model) = parse_agent(agent);   // already called
if let Some(kind) = crate::harness::HarnessKind::parse(&harness) {
    crate::journal::record_provider(kind.as_str());
}
if let Some(model) = model {
    crate::journal::record_model(&model);
}
```

`parse_agent` (`engine/config.rs:263`) already returns `(String, Option<String>)`
and defaults claude → `opus`.

## 4. Storage — `lfdb`

- `RunEventRow` (`lfdb/mod.rs`): add `process_id: String` (required — the column
  is `NOT NULL` and the writer always knows it), `parent_process_id:
  Option<String>` (None at the root), `model: Option<String>`.
- `insert_run_event` and both `SELECT`s (`sqlite.rs`, ~1373/1405/1417) and
  `query_run_events`'s mapper: add the three columns. Keep column order aligned
  with the mapper indices — this is where a mistake goes unnoticed.
- Update the `usage_row`/`event_row` test helpers that construct `RunEventRow`.

## 5. Readers

### `lf runs` — `lf/commands/runs.rs::summarize`

Today it groups by `run_id`, takes the label from the **first** event and the
tokens from the **last** terminal row. For run `66863649` (nine commands) that
prints the wave's name beside a nested `lf op pm show`'s failure and zero
tokens. Every displayed number for a nested run is a splice of two processes.

Group by `process_id`. One summary per process. Label from that process's own
`command`. Tokens, cost, and status from that process's own terminal row. A
run's total is the sum over its processes — additive, because each process
reports its own cumulative total.

### `lf trace <run_id>` — same file

Render the process tree: root spans (`parent_process_id IS NULL`) and their
children, indented. This is the flamechart's data, in text.

### `lf doctor` — `lf/commands/doctor.rs`

There is no legacy path: every row has a `process_id`. `lf doctor` must exit 0
after this pass, and stay there. A monitor that is permanently red is a monitor
people learn to ignore — that is how a 29-hour outage went unnoticed.

- `check_attribution`: within a single `process_id`, >1 distinct command is a
  `Fail`. Across a `run_id` it is expected and fine. Terminal rows that name no
  work: `Fail`.
- `check_identity`: a `repo` that is not an absolute path is a `Fail`. Delete
  `looks_like_temp_root` — after 057 a temp root is a legitimate absolute path,
  and the heuristic was only ever a symptom detector for the basename bug.
- `check_vocabulary`: green by construction now. Keep it — it is the tripwire
  for the next half-landed rename, and the CHECK constraint only guards writes
  from this binary.
- `check_continuity`, `check_coverage`: unchanged.
- Add `check_lineage`: every `parent_process_id` resolves to a known
  `process_id`, or is NULL (a root). A dangling parent means the tree cannot
  render, and the flamechart would silently drop a subtree.

### `lf usage`

No change. It sums terminal rows, which stay additive per process.

## 6. The API the dashboard consumes

Add `--json` to `trace` and `doctor` (`runs` already has it). These are wire
types: **no defaults, no `#[serde(default)]`, every field required or
explicitly `Option`** (STYLE.md, "DTOs").

```rust
pub struct SpanDto {
    pub run_id: String,                   // the trace
    pub process_id: String,               // the span
    pub parent_process_id: Option<String>, // None at the root
    pub node: String,                     // run | flow | skill
    pub name: Option<String>,             // command, flow, or skill
    pub started_at: i64,
    pub ended_at: Option<i64>,            // None when the frame never closed
    pub status: String,                   // completed | errored | escalated | open
    pub input_tokens: Option<i64>,        // cumulative at this boundary
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cost_usd: Option<f64>,
    pub duration_secs: Option<f64>,
    pub provider: Option<String>,
    pub model: Option<String>,
}
```

Two rules the dashboard must not have to rediscover:

- **Frames may not close.** A crashed or killed process leaves a `started` row
  with no terminal row. A span with `ended_at: None` is `status: "open"` —
  render it to its last known timestamp, never to "now", and never drop it. A
  chart that hides unclosed frames hides exactly the runs that went wrong.
- **Values are cumulative.** A span's *own* cost is
  `cost_usd - previous_boundary.cost_usd` within the same `process_id`. Provide
  `pub fn own_spend(spans: &[SpanDto]) -> Vec<SpanDto>` in the reader so no
  consumer reimplements the diff, and so the rule has exactly one home.

## 7. Tests

Name the regression, not the function.

- `a_nested_lf_gets_its_own_span_and_names_its_parent` — two contexts, child's
  `parent_process_id` == parent's `process_id`, distinct `process_id`s.
- `a_terminal_row_names_the_work_its_started_row_named` — no join needed.
- `two_processes_sharing_a_run_id_summarize_separately` — `summarize` returns two
  rows for run `66863649`-shaped input, each with its own label and its own cost.
- `a_span_that_never_closed_is_open_not_zero_width`.
- `own_spend_diffs_consecutive_boundaries_within_a_process`.
- `a_closed_vocabulary_rejects_an_unknown_node` — inserting `node='task'` errors.
- `a_dangling_parent_process_id_fails_the_doctor`.
- `the_migration_starts_the_ledger_empty` — seed a db with pre-057 rows, apply,
  assert `run_events` is empty and `process_id` is `NOT NULL`.

Migration tests must seed a **drifted** db, not only a fresh one. Every existing
migration test builds from 001 forward, which is exactly why CI stayed green
while the only machine holding real history was broken.

## 8. Done when

Not "the columns exist." Each of these is observable, and each one is a thing a
person can see working. Do not stop early — a telemetry pass that lands schema
and no chart has produced no product value, and the next pass will inherit an
untested API.

**The ledger is trustworthy**

1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
   all pass.
2. `lf doctor` exits **0** on the real ledger. Every check reads `ok`. Paste the
   output into the PR.
3. Deliberately corrupt it and watch it fail: `sqlite3 ~/.lf/lfd.db "insert into
   run_events (run_id, process_id, seq, ts, node, event) values ('x','y',0,0,'task','started')"`
   is rejected by the CHECK constraint. Then hand-insert a row with a
   `parent_process_id` that resolves to nothing, and confirm `lf doctor` fails
   `lineage` and exits 1. Delete the row.

**A run explains itself**

4. `lf : "reply ok"` — then `lf runs` shows that run with its own command,
   tokens, cost, provider, **and model**, and `lf usage` moves by exactly that
   run's cost.
5. `lf wave <name>` for one iteration, then `lf trace <run_id>` prints a tree:
   the wave at the root, each nested `lf` as a child span with its own cost. The
   sum of the children's costs plus the root's own spend equals the run total.
   **This is the assertion that proves the whole pass** — it is the exact thing
   that was impossible before, when nine commands shared one id.
6. Kill a run mid-flight (`Ctrl-C` a `lf :`). `lf trace` shows that span as
   `open`, not as zero-width and not missing.

**The dashboard is real**

7. `lf trace <run_id> --json | jq` returns the `SpanDto` tree; no two spans share
   a `process_id`; every `parent_process_id` resolves.
8. The **run flamechart renders from a real `lf wave` run** — nested spans at the
   right depth, widths equal to elapsed time, open frames hatched. Screenshot in
   the PR.
9. The **cost waterfall** for that same run sums, bar for bar, to the run total
   `lf usage` reports for it. If they disagree, the cumulative-diff rule is wrong
   and the pass is not done.
10. The **silence ribbon** renders across the last 7 days, and shows the ledger's
    real coverage. Verify it works by stopping the ledger: point `LF_HOME` at a
    scratch dir, run nothing for a minute, and confirm the ribbon goes black for
    that window rather than interpolating over it.

**It stays true**

11. `lf doctor` is wired into the intelligence wave's `daily` cron, so a silent
    outage surfaces the next morning instead of 29 hours later.
12. `TESTING.md` names the drifted-db migration test as the guard against the
    class of bug that started all of this.

## 9. First dashboard slice

Build the **single-run flamechart**, per `dashboard-design.md`'s open question:
the PR view is the better product surface but is blocked on the delivery record,
while the run view is blocked only on what this pass delivers.

Four charts, all computable the moment this lands:

1. **Run flamechart.** `x` = wall-clock, nesting = the `parent_process_id` tree,
   width = `ended_at - started_at`, color = own cost. Open frames hatched, not
   hidden. Preserve waiting and parallelism: summed span durations are not
   elapsed run time.
2. **Cost waterfall.** One bar per skill in a run, `own_spend` diffed from the
   cumulative rows. Answers "where did the money go" in one glance — the thing a
   flamechart is bad at because narrow frames can be expensive.
3. **Cache-hit ratio over time.** `cache_read / (input + cache_read)`, per day
   and per skill. Before the truncation the ledger showed ~76M cache-read
   against ~207M input tokens over five days: cache behaviour is the largest
   cost lever in the system and nothing plots it. Needs a few days of post-057
   data to be interesting; build it anyway, it is three lines once `own_spend`
   exists.
4. **Silence ribbon.** A coverage strip across the period, black where the ledger
   recorded nothing. This is `lf doctor`'s continuity check, drawn. It makes the
   dashboard's own trustworthiness the first thing you see — the 29.2-hour
   outage would have been a black band, not a mystery.

Movement metrics (PRs landed, cost per landed PR, lead time, interventions) stay
out until the delivery record and `escalated` land. Do not synthesize them from
`runs.snapshot_pr`: 7 of 171 rows have one, all from `break-test`.
