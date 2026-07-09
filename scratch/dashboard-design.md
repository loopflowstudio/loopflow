# Ledger dashboard

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

## Flamechart

Render the recorded run -> flow -> skill hierarchy across wall-clock time.
Width is elapsed time. Color is cost or token intensity. Every frame shows
input, output, cache-read tokens, cost, status, harness/provider, and model when
the ledger records it.

The chart must preserve waiting and parallelism. Summed skill durations are not
the same thing as elapsed run time.

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
