# Ledger dashboard

> **Status, measured 2026-07-09.** The ledger is now a reliable API: span
> identity, closed vocabularies, absolute repo, model, cumulative usage, and an
> `lf doctor` that exits 0. `lf runs --json`, `lf trace <id> --json`, and
> `lf doctor --json` are the surfaces this page consumes.
>
> The first slice is drafted in `swift/LoopflowMac/Views/TelemetryDashboardView.swift`
> — run flamechart, cost waterfall, cache-hit ratio, silence ribbon — and it
> compiles, but **it has never rendered real data**. Finishing that is the whole
> remaining risk; see `plan.md`. Demo instructions live there too (`swift run`
> crashes: the app needs a real bundle, so build with xcodegen + xcodebuild).
>
> The PR metrics below remain blocked on a delivery record and a firing
> `escalated` event. Neither is telemetry work.

> "PRs by wave would be cool"

> "Token flamechart for main, and some sort data about movement over time would
> be cool"

## Direction

The dashboard treats a wave as the unit of investment and a landed PR as the
unit of output. Its home screen answers: what is moving, what shipped, and what
did it cost? A run flamechart explains the outliers.

## Home

One row per wave:

- landed PRs in the selected period, plus open/in-flight PRs;
- total cost, agent wall-clock, and human intervention count;
- median cost and wall-clock per landed PR;
- movement against the previous equal-length period;
- freshness: last run, last landed PR, and days with no ledger coverage.

The row expands into a time series of runs and PR landings. A PR opens the runs
that contributed to it; a run opens its flamechart.

## The four charts

All four read one payload: `lf usage --json --days 30`, which emits one row per
*boundary* — what each skill, and each inline run, actually spent — with the
cumulative-diff rule already applied. The rows are additive and sum to the
totals `lf usage` prints. **That reconciliation is the contract**; a chart that
disagrees with the table is wrong, and `boundary_spend_sums_to_the_run_total_without_double_counting`
pins it.

1. **Tokens by skill, 30 days.** Stacked per day. A boundary with no skill is an
   inline prompt and is labelled as such rather than dropped.
2. **Tokens by model, 30 days.** Same shape, keyed by `provider:model` — the
   harness and the model it drove.
3. **Token flame, by repo.** `repo → wave → flow → skill`, width proportional to
   tokens. **Width is tokens, not time.** A fast skill can be the widest thing on
   the page; that is the point of a cost chart, and it is why this replaced a
   wall-clock flamechart.
4. **Cache-hit ratio.** `cache read / (input + cache read)`, per boundary, keyed
   by agent. The largest cost lever in the system.

A dimension nothing in a subtree carries (a repo with no waves, say) is skipped
rather than drawn as a row of `—`.

## Movement

Trend weekly by default, with 7/30/90-day windows:

- PRs landed by wave;
- cost and wall-clock per landed PR;
- first-pass completion and human interventions per PR;
- token mix by harness/model and by skill;
- active-to-landed lead time;
- ledger coverage and unattributed spend.

Show absolute values and change versus the previous period. Avoid a synthetic
productivity score: the dashboard should expose the tension between throughput,
cost, time, and intervention rather than hide it in weights.

## Ledger reality

> Measured against `~/.lf/lfd.db` on 2026-07-09, and the optimistic reading
> below did not survive it. Full audit in `telemetry.md`; run `lf doctor`.

`run_events` records wave, run/flow/skill lifecycle, duration, provider, tokens,
and cost. That is **not yet** enough for a useful flamechart, for four reasons,
each measured:

- **Frames do not close.** 2474 `run/started` against 2169 terminal rows; flow
  72/43; skill+step 260/209. Between 12% and 40% of frames have a start and no
  end. A frame without an end has no width.
- **The hierarchy is inferred, not recorded.** There is no parent link. `run_id`
  names a *tree of processes* — a nested `lf` inherits `LF_RUN_ID` — and 134
  run_ids carry more than one command (one carries nine). Rendering run→flow→
  skill from a shared `run_id` puts nine sibling roots in one chart.
- **The layer names are split.** `node='step'` (309 rows) and `node='skill'`
  (148 rows) are the same concept either side of migration 054. Any grouping
  drops half the history.
- **Cost and tokens had different accumulation rules.** Tokens and duration
  accumulated; `cost_usd` overwrote. So per-frame cost could not be diffed out,
  and 24 runs recorded a cost that *fell* between skills. Fixed: every usage
  field is now cumulative-to-that-point, so a frame's own cost is the diff of
  consecutive rows. **Historical rows predate the fix and cannot be diffed.**

Also: `wave` is present on only 3378/5227 rows (65%), so "by wave" silently
drops a third of the data.

PR attribution is not yet trustworthy. The registered `runs` table carries a
PR snapshot and joins to a wave, but the long-lived ledger currently has only
seven such rows and all are from `break-test`. Ordinary `lf pr open`, `submit`,
and `land` invocations appear in `run_events` without a durable PR number or
merge event. The dashboard therefore needs an explicit run/wave/PR delivery
record before "PRs by wave" can be a real metric.

Model is also absent from `run_events`; provider/harness exists (added
2026-07-09, so every historical row has `provider = NULL`). A harness/model
split requires recording model on the terminal agent-bearing event rather than
inferring it from command text or prompt logs.

**Human interventions do not exist as data.** `LfEventType::Escalated` is
defined and emitted zero times in 5247 rows. "Human interventions per PR" and
"first-pass completion" have no source. Both need an event, not a query.

## What the dashboard therefore demands

The dashboard is the best forcing function we have for telemetry, because it
names the metrics and each one indicts a missing invariant. Ordered by what
unlocks the most:

1. **`process_id` / `parent_process_id`.** Unblocks the flamechart's hierarchy,
   unambiguous cost attribution, and a truthful `lf runs`. Nothing else on this
   page works without it.
2. **A delivery record** (run/wave → PR number → merge event). Unblocks every
   PR metric on Home and in Movement: landed PRs, cost per landed PR, lead time.
3. **Closed `node`/`event` vocabularies + terminal rows that name their work.**
   Unblocks "token mix by skill" and stops history from splitting again.
4. **An `escalated` event that actually fires.** Unblocks intervention counts.
5. **Model on the terminal agent event.** Unblocks the harness/model split.

Freshness, ledger coverage, and unattributed spend — the Home row's last column
— already exist: that is `lf doctor`, and it currently exits 1.

## Open detail

Decide whether the main flamechart represents one selected run, every run that
contributed to one PR, or the whole selected period. The first is operationally
clean; the PR view is probably the more revealing product surface.

**Resolved by the data, for now:** build the single-run flamechart first. The
PR view is the better product surface and is blocked on the delivery record
(7 PR snapshots in 171 `runs` rows, all from `break-test`), whereas the run view
is blocked only on `process_id` and closing frames — both of which the PR view
also needs. Ship the run view, earn the PR view.
