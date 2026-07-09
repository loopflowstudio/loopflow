# Telemetry: define, ensure, monitor

The ledger is the wave's only evidence. Everything downstream — trace, evals,
cost attribution, "did loopflow move the portfolio" — is a query against it. It
is currently not trustworthy, and the failures are structural rather than
incidental.

Measured on the real ledger (`~/.lf/lfd.db`, 2026-07-09), not reasoned about.

## What went wrong, and why none of it was noticed

| # | Defect | Evidence |
|---|---|---|
| 1 | **Writes fail silently** | `ledger_insert` degraded errors to `debug!`. The `step_index` drift broke `insert_run_event`, and 29.2 hours of runs vanished (2026-07-08 14:59 UTC → 2026-07-09 20:12 UTC), every repo. Readers failed loudly the whole time. *Fixed: first failure per process now `warn!`s.* |
| 2 | **`run_id` names a tree, readers treat it as a run** | A child `lf` inherits `LF_RUN_ID` (by design, per `047_run_events.sql`). 133 run_ids carry >1 distinct started-command. Run `66863649` has **9**. |
| 3 | **Terminal rows carry no identity** | Of 2150 terminal run rows: `flow` NULL on 2150, `skill` NULL on 2150, `command` NULL on 2150. `command` lives only on `run/started` (2456 rows, all populated). |
| 4 | **`node` vocabulary drifted** | `step` (309 rows, ends 2026-07-08 01:36) and `skill` (148 rows, starts 2026-07-09 20:16) are the same concept. Migration 054 renamed the *column* and left the *values*. |
| 5 | **`repo` is a basename, not an identity** | 898 distinct values; 888 are temp-dir-shaped (`tmp.Rf5ZtVARiJ`, `.tmpzt80hK`); plus literals `repo`, `src`, `demorepo`. Three are real projects. Derived from `main_repo.file_name()`. |
| 6 | **Cost overwrote instead of accumulating** | `record_usage` did `+=` on tokens and `record_result` did `+=` on duration — but `=` on `cost_usd`. 24 runs record a cost that *decreases* between skills, which no running total does. A multi-skill run stored only its last agent invocation's cost, so `lf usage`'s `SUM(cost_usd)` undercounted across 28 multi-skill runs. *Fixed: cost accumulates.* |

### The bug these compose into

`lf runs` labels a run from the **first** event it finds and takes tokens and
status from the **last** terminal row (`summarize`, `runs.rs:284`). For a shared
`run_id` those are different processes. Run `66863649` is displayed as
`wave architecture — errored, no tokens`: the label is the wave, the status and
cost belong to a nested `lf op pm show` that spent nothing and failed.

Every number in `lf runs` for a nested run is a splice of two unrelated
processes. This is not a display bug. There is no correct thing to display,
because the grain does not exist in the data.

## Define — the contract

A run event is one row. The contract every row must satisfy:

1. **Identity is a span, not a name.** `run_id` is the *trace* — the whole tree,
   stable across nested `lf`. Add `process_id`, minted per process, and
   `parent_process_id`. A process's `started` and terminal rows share a
   `process_id`; a nested `lf` gets a fresh one whose parent is the caller's.
   Attribution then joins inside a process and is unambiguous.
2. **A terminal row is self-describing.** It carries the same `command`, `flow`,
   and `skill` as its process's `started` row. A reader must never need a join
   to say what a cost was spent on.
3. **`node` and `event` are closed vocabularies.** `node ∈ {run, flow, skill}`,
   `event ∈ {started, completed, errored, escalated}`. Enforced by a CHECK
   constraint, so a rename cannot half-land again.
4. **`repo` is a stable identity**, not a display string. The main repo's
   absolute root (or a `repo_id`), with `worktree` holding the actual directory.
   Basenames collide and temp roots mint garbage.
5. **Spend is attributed to the process that spent it.** Tokens and cost belong
   to the `process_id` whose agent stream reported them. Summing across a
   `run_id` is then additive and correct by construction.
6. **Every usage field on a row is cumulative to that point.** Tokens, cost, and
   duration all accumulate; a reader diffs consecutive rows for a per-skill
   figure and reads the terminal row for the run total. One rule, no per-field
   exceptions — cost was the exception and it silently undercounted.

## Ensure — the write path cannot lie

- A ledger write may never fail a run, but it may never fail *quietly* either.
  First failure per process warns. *(done)*
- **Schema guard at open.** `SqliteStore::new` asserts the columns the code
  reads and writes exist. A ledger whose schema disagrees with the binary is a
  loud error, not 29 hours of silence.
- **A migration test that starts from a drifted db**, not only a fresh one.
  Every existing migration test builds from 001 forward, which is exactly why
  CI was green while the only machine holding real history was broken.
- Closed vocabularies get CHECK constraints; renames get a data migration for
  historical values, not just a column rename.

## Monitor — the ledger reports on itself

`lf doctor` answers, on the real ledger, the questions nobody was asking:

- **Continuity.** Gap-days *and longest silence*. The first version of this
  check measured only gap-days and reported the real ledger healthy: the
  29.2-hour outage began mid-day and ended mid-day, and both days held rows.
  A gap-day check would **not** have caught it. Silence would, and does:
  `warn continuity … 29.2h of silence — was the ledger listening?`
  The doctrine applies to the doctor: run the reader before trusting it.
- **Attribution.** How many run_ids carry >1 command; how many terminal rows
  lack `command`/`flow`/`skill`.
- **Vocabulary.** Any `node`/`event` value outside the closed set.
- **Identity.** How many distinct `repo` values look like temp roots.
- **Coverage.** What fraction of agent-bearing runs carry tokens and provider.
- **Schema.** Whether the ledger's columns match what this binary expects.

This is what the `daily` cron should run. It is also the only one of these legs
that pays off before the others land: it turns each defect above from a thing
someone happens to notice into a number that moves.

## Order

1. `lf doctor` — monitor first. It costs nothing, needs no migration, and it
   measures the other four defects while they are still broken.
2. Schema guard + drifted-db migration test — *ensure*, closes the 29-hour class.
3. `process_id` / `parent_process_id` + self-describing terminal rows — *define*,
   the one that makes `lf runs`, `lf trace`, and cost attribution actually true.
4. `repo` identity + closed vocabularies with CHECK constraints.

Evals depends on all of it: a harness that reads a deaf ledger reports zeros and
looks healthy.
