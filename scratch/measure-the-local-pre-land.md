# Measure every local pre-land gate

## Problem

`uv run python scripts/test.py --all` is the documented full local pre-land
gate. It already bounds every phase with a named timeout, but it discards the
measurements when the process exits:

- `_run_command()` measures elapsed time only to explain a timeout.
- `_run_suite()` retains one aggregate duration for the suite, so the summary
  cannot say how long `rustfmt`, `clippy`, or `rust` took individually.
- no run survives terminal output, so the Developer Efficiency KR cannot be
  judged over a month.

The existing ops metric file is the wrong authority for this proof. It is a
best-effort JSONL file inside each worktree at
`.lf/tmp/metrics/ops.jsonl`. Loopflow prunes clean terminal Task worktrees, and
temporary-tree cleanup may remove `.lf/tmp` independently. Either event erases
the history. Reusing that file would make a month of green landings look
identical to a month with no evidence.

This work gives developers and the infrastructure Wave a durable answer to:
which phase ran, what named budget governed it, how long it took, and whether
the last 30 days stayed inside every budget. It advances the Project KR:
“The full local pre-land verification path has measured budgets and stays
inside them for a month of landings.”

## The demo

Run the full gate, then read its durable evidence:

```bash
uv run python scripts/test.py --all
uv run python scripts/test.py --history 30
```

The ordinary summary shows every phase as `elapsed / named budget`. The history
view prints one row per recorded full pre-land run, names any over-budget or
incomplete phase, and ends with `IN PROGRESS`, `NOT HOLDING`, or `HOLDING` for
the 30-day observation window. Removing `.lf/tmp` between the two commands does
not change the history.

## Approach

Keep measurement inside `scripts/test.py`, the process that owns the phase
clock and budgets. Add a small, gate-specific history model rather than a
general telemetry service.

### Return a typed outcome for every phase

Replace `_run_command()`'s `Optional[str]` result with a `PhaseOutcome` that
always carries:

- suite and phase labels;
- the named `budget_s` selected from `PHASE_BUDGETS`;
- monotonic `elapsed_s`;
- status: `passed`, `failed`, `timed_out`, `missing_tool`, or `not_run`;
- `over_budget`, computed by the runner rather than inferred by a reader;
- the existing actionable failure text when present.

`SuiteOutcome` retains its ordered phase outcomes. At plan construction, every
selected phase starts as `not_run`; after each command, its record is replaced
with the observed outcome. If an earlier command stops a suite, later phases
remain explicitly `not_run` rather than disappearing. Suite and total elapsed
times remain useful summaries, but they are derived from phase outcomes and do
not substitute for them.

The terminal summary prints phase rows before suite totals. A timeout therefore
reads as, for example, `TIMEOUT xcodebuild 1211s / 1200s budget`, with no manual
subtraction or log timing.

### Persist one atomic record per invocation

For each verification invocation, attempt to create one JSON record before the
first selected phase runs and checkpoint it after every phase boundary. A
`full` run requires that creation to succeed; non-full runs follow the warning
path below. Store successful records under:

```text
<git-common-dir>/loopflow/pre-land/runs/<run-id>.json
```

Resolve `<git-common-dir>` with:

```bash
git rev-parse --path-format=absolute --git-common-dir
```

The common directory is shared by all linked worktrees and lives outside every
Task worktree. Loopflow's terminal-worktree pruning removes the Task worktree,
not the common Git directory. The record therefore survives Task cleanup,
`.lf/tmp` cleanup, and machine restart without touching a tracked file.

Each checkpoint writes a sibling temporary file, flushes it, and atomically
replaces the run file. One file per invocation avoids cross-worktree append
contention and makes a crash leave either the prior complete checkpoint or the
new one, never an interleaved JSONL line. The record is created with status
`running`; a normal exit changes it to `passed` or `failed`. A process or
machine killed between checkpoints remains visibly incomplete rather than
vanishing from the denominator.

History persistence is part of the `full` gate contract, not best-effort
telemetry. If a full run's initial record or phase checkpoint cannot be
written, the runner exits nonzero with `MEASUREMENT FAILED` and the resolved
path. A passing landing matrix must not claim a measured pass after dropping
the evidence the KR reads.

The ordinary `changed` loop and separately invoked `required_host` gate still
attempt the same records, but persistence failure prints one visible
`MEASUREMENT WARNING` and leaves their underlying test exit unchanged. After
the first write failure, that invocation disables further checkpoints rather
than repeating the warning after every phase. These records are useful
diagnostics, not part of the 30-day denominator, so losing one must not tax the
fast loop or obscure a hosted test result.

Use a versioned, fixed schema containing only operational facts:

```json
{
  "schema": 1,
  "run_id": "20260717T120102Z-41203-8f3a2c1d",
  "kind": "full",
  "branch": "jack-heart/example",
  "head": "0123456789abcdef",
  "task_session_id": "ts_...",
  "started_at": "2026-07-17T12:01:02Z",
  "finished_at": "2026-07-17T12:07:31Z",
  "status": "passed",
  "phases": [
    {
      "suite": "rust",
      "phase": "clippy",
      "budget_s": 900,
      "elapsed_s": 41.238,
      "status": "passed",
      "over_budget": false
    }
  ]
}
```

`task_session_id` is optional and copied only when Loopflow already supplied
`LF_TASK_SESSION_ID`; branch and HEAD are read from Git. The record never
contains commands, output, file paths, diffs, environment contents, prompts, or
secrets. Failure logs and `.xcresult` bundles remain under the existing
reapable `.lf/tmp/gate/run-<pid>/` path because they are repair artifacts, not
endurance evidence.

Build `run_id` from UTC start time, pid, and a `secrets.token_hex(4)` suffix.
The random component prevents a fast process-id reuse after restart from
overwriting an earlier run at the same timestamp.

Attempt to record every verification invocation; `--list` and the read-only
`--history` mode do not create records and cannot be combined with
run-selection flags. Mark `kind` as:

- `full` for `--all`, the documented landing-verification unit;
- `changed` for the ordinary changed-aware loop;
- `required_host` for the separately invoked `--ui-host` gate.

The 30-day KR view defaults to `full` records. It does not pretend that
`--all` executed the separately named hosted UI gate. Changed-aware and host
runs remain available as evidence but do not inflate the full-gate count.

### Add a reader that judges the budget window

Add `--history DAYS` to the same script. It performs no verification work; it
reads records from the shared history directory and prints:

- timestamp, branch, abbreviated HEAD, gate result, and total elapsed/budget;
- every phase whose `over_budget` is true;
- incomplete or unreadable records as evidence gaps, never as passes;
- observation-window age and a final verdict.

Verdict rules scan all schema-1 records to find the clock start, then judge only
records whose start falls inside the requested trailing window:

- `IN PROGRESS`: all complete full runs are inside budget, but the oldest
  schema-1 full run is less than `DAYS` old;
- `NOT HOLDING`: any full run in the window has an over-budget phase, an
  incomplete measurement, or an unreadable record;
- `HOLDING`: the clock started at least `DAYS` ago, the trailing window contains
  at least one complete full run, and every full run in that window has
  complete phase evidence with no over-budget phase.

Test failures and budget failures stay separate. A test assertion may fail
well inside its time budget; that is a red gate, but it is not fabricated into
a budget overrun. Conversely, a timeout is both a red gate and an explicit
budget failure: `timed_out` always sets `over_budget: true`, independent of
sub-second clock rounding near the boundary.

`--history` reports verification runs, not GitHub merge truth. In this
repository `--all` is already the documented final pre-land unit and the Task
gate requires the full matrix before final ship. Branch, HEAD, and optional
Task Session identity make each observation attributable without adding a
GitHub query or turning `lf pr land` into a repository-specific test runner.

### Keep budgets legible and historically truthful

`PHASE_BUDGETS` remains the executable source of truth. Every record copies the
budget that governed that run, so changing a budget later cannot rewrite old
evidence. Keep `release/GATE_BUDGET.md` as the reader-facing list and add a test
that every named `PHASE_BUDGETS` entry and value appears in its per-phase table.
`TESTING.md` gains the history command and the durable-path/retention contract.

Do not write gate evidence through `ops_metrics_path()`. That helper remains
the single owner of the existing best-effort ops telemetry path; hardcoding its
`.lf/tmp` location anywhere new would repeat W2-233's defect. Gate history has
different correctness and retention semantics, so it gets one narrowly named
`_gate_history_dir()` resolver instead of masquerading as another ops event.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Where are the current budgets? | `scripts/test.py::PHASE_BUDGETS` names 11 command phases; `release/GATE_BUDGET.md` documents the same values. | Keep the map authoritative, copy the applied value into each historic phase, and guard the doc/map pairing with a test. |
| Does the runner currently retain phase durations? | No. `_run_command()` returns only failure text and `_run_suite()` collapses all command time into one suite duration. | Introduce `PhaseOutcome`; do not try to reconstruct stage timing from suite totals or logs. |
| Can existing ops telemetry carry a month? | No. `ops_metrics_path()` resolves inside each worktree's ignored `.lf/tmp`; terminal worktree pruning removes the entire worktree, and temp cleanup may remove the file sooner. The writer is also intentionally best-effort. | Reuse the privacy/event-shape idiom, not its authority or retention. Keep the existing helper untouched and give `full` gate proof a durable, fail-loud path. |
| What local path survives linked-worktree removal? | `git rev-parse --path-format=absolute --git-common-dir` resolves `/Users/jack/src/loopflow/.git` from this Task worktree, while the Task itself lives at `/Users/jack/src/loopflow.measure-the-local-pre-land`. `main_repo_root()` already derives the canonical checkout from this same Git common directory. | Store evidence beneath the common directory, shared by all Task worktrees and outside their prune target. |
| Will durable records recreate the disk-pressure incident? | Current ops telemetry across the inspected live worktrees is 30 events and under 8 KiB. A gate record has a fixed schema, no logs, and roughly 11 short phase objects; even 10,000 retained runs are tens of MiB, not the hundreds of GiB caused by temporary trees and build artifacts. | Retain the small evidence indefinitely for now; keep logs/build artifacts reapable. Add no retention service until measured volume warrants one. |
| Can concurrent worktrees corrupt one history file? | A shared append-only JSONL file would introduce a new cross-worktree writer race; the current ops file avoids it only because it is per-worktree. | Use one atomically replaced file per run. Readers scan files; writers never share one append cursor. |
| What happens on power loss or SIGKILL? | A single write at process exit can disappear entirely and create survivor bias. | Create the run before executing phases and checkpoint after each phase; stale `running` records are explicit evidence gaps. |
| Should evidence writes be best effort? | It depends on the record's authority. A silently dropped `full` record lets an unmeasured landing month look green; a dropped `changed` or `required_host` diagnostic does not enter the KR view. | Fail `full` runs loudly with `MEASUREMENT FAILED`; warn once and preserve the underlying exit for non-full runs. |
| Does `--all` include the hosted UI gate? | Deliberately no. `ui-host` requires a permissioned macOS host and is separately named in the plan and docs. | Preserve `required_host` as a separate record kind; never count it as a phase that `--all` ran. |
| Does this need a shared service or SQLite migration? | No. The writer and reader are one stdlib Python process, the evidence is local, and the required query is a bounded directory scan. The live registry has also experienced write contention; coupling a repository test runner to it would add risk without improving the KR proof. | Use local atomic files; no daemon, migration, network, or generic metrics API. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Append gate events to `.lf/tmp/metrics/ops.jsonl` via the existing telemetry path | Smallest code change and matches the rebase event shape. | It fails the defining retention requirement: per-worktree evidence disappears with Task pruning or temp cleanup, and best-effort drops are silent. |
| Move all ops telemetry out of `.lf/tmp` and make it authoritative | One shared metrics stream for rebase, worktree, gate, and land events. | It changes retention and concurrency semantics for unrelated operations, turns this Task into a telemetry redesign, and makes concurrent worktrees share an append file. |
| Add gate tables to `~/.lf/loopflow.db` | Durable, queryable, and naturally attached to Loopflow. | `scripts/test.py` is a repo script, not a store client. A schema migration plus cross-language writer would be larger than the feature and would add writes to a registry with measured contention. |
| Append to a tracked `.lf/metrics/` file or commit monthly evidence | Git preserves it and reviewers can see it. | Every run dirties the checkout, repeats the W2-233 regression, and creates merge conflicts across Task worktrees. |
| Store one JSONL file under `~/.lf/metrics/` | Survives worktree pruning and restart. | It multiplexes unrelated clones, must reproduce Loopflow's dev/control-home routing, and needs locking for concurrent writers. Git's common directory already gives this repo a durable shared local namespace. |
| Emit only the improved terminal summary | Almost no persistence code. | It fixes manual timing for one run but leaves the month-long KR unmeasurable after the terminal scrolls away. |

## Key decisions

- **A full `--all` invocation is the landing-verification observation.** The
  existing docs and gate skill already define it as the final local matrix.
  This Task measures that path; it does not make `lf pr land` invoke
  repo-specific tests or prove GitHub merge state.
- **Only the proof path is load-bearing.** A `full` durable-write failure is a
  gate failure, not a warning, because otherwise the KR history is biased
  toward records that happened to persist. `changed` and `required_host`
  writes degrade visibly without changing their test result because the KR
  never reads them.
- **History is immutable evidence, budgets are captured values.** A future
  budget edit affects future runs only.
- **Incomplete is a first-class state.** Absence after a crash does not become
  success, and phases skipped after an earlier failure do not disappear.
- **One run, one file.** This keeps the concurrency model boring and avoids a
  lock, database, or append-repair tool.
- **Keep heavy artifacts temporary.** Only compact proof survives; logs,
  DerivedData, and `.xcresult` bundles remain reapable.
- **No generic telemetry platform.** The code knows about gate runs, phases,
  budgets, and a history verdict—nothing more.

Wild success is mundane: after 30 days the Wave runs one command and sees the
exact phases, budgets, overruns, and observation age that make the KR hold.
Developers use the same view to notice clippy or xcodebuild drifting weeks
before it reaches the timeout.

Wild failure is one policy applied beyond its authority: either a reassuring
green landing report assembled only from surviving successful exits, or an
unwritable history directory blocking every changed-aware test loop while
buying no KR evidence. The create-first/checkpoint-often full record and its
fail-closed persistence prevent the false green; the non-full warning path
keeps optional diagnostics from becoming developer friction.

## Scope

- In scope:
  - per-command elapsed time and budget classification in `scripts/test.py`;
  - improved phase-level terminal summaries;
  - atomic gate records under the Git common directory;
  - `--history DAYS` with an explicit 30-day KR verdict;
  - full, changed-aware, and required-host record kinds;
  - tests for timeout, failure, skipped phases, atomic persistence, temp-tree
    deletion, incomplete runs, full-vs-non-full write failure, history verdicts,
    privacy, and budget-doc drift;
  - `TESTING.md` and `release/GATE_BUDGET.md` updates.
- Out of scope:
  - a generic telemetry API, daemon, dashboard, or database migration;
  - moving or changing existing rebase/worktree ops telemetry;
  - retaining phase logs or build artifacts for a month;
  - changing the budgets themselves;
  - making `lf pr land` run this repository's test script;
  - querying GitHub to prove that a recorded branch eventually merged;
  - cross-machine aggregation or syncing local evidence;
  - folding the separately required `ui-host` gate into `--all`.

## Done when

1. `uv run python scripts/test.py --all` prints and persists each selected
   phase's label, actual duration, applied named budget, status, and
   `over_budget` verdict.
2. A phase that crosses its budget is killed as today, prints a named
   `TIMEOUT ... elapsed / budget`, and appears as `timed_out` plus
   `over_budget: true` in history.
3. A failure before later phases records those phases as `not_run`; a killed
   runner leaves an incomplete record that the history view refuses to call
   green.
4. Deleting the whole worktree-local `.lf/tmp` tree after a run leaves
   `uv run python scripts/test.py --history 30` unchanged.
5. A fixture writes history from two simulated linked worktrees into the same
   common-directory root without collision, then removes one worktree fixture
   and still reads both records.
6. `--history 30` names every over-budget or incomplete full run and reports:
   - `IN PROGRESS` before 30 days of evidence,
   - `NOT HOLDING` for an overrun or evidence gap,
   - `HOLDING` only after a complete 30-day window with no overruns.
7. Records contain no command output, argv, cwd, diffs, environment payloads,
   prompts, or secrets.
8. A test proves every `PHASE_BUDGETS` label/value is stated in
   `release/GATE_BUDGET.md`.
9. With an unwritable history root, `--all` exits nonzero and prints
   `MEASUREMENT FAILED`; the ordinary changed-aware run prints one
   `MEASUREMENT WARNING` and returns the underlying test result unchanged.
10. Focused verification passes:

   ```bash
   uv run pytest python/tests/test_gate_bounded.py python/tests/test_release_automation.py -v
   uv run python scripts/test.py --list --all --base HEAD
   ```

11. The implementation's own demo uses a temporary history root in tests to
    prove 29 days => `IN PROGRESS`, 30 clean days => `HOLDING`, and one named
    overrun => `NOT HOLDING` without running the multi-hour matrix.

## Measure

Baseline from the current tree:

- persistent phase records per gate run: **0**;
- individually retained phase durations: **0 of 11** named phases;
- month-window verdict: **unavailable**;
- existing inspected ops telemetry: **30 events, under 8 KiB**, fragmented
  across live worktrees.

After this lands:

- persistent phase records per selected gate run: **all selected phases**;
- individually retained full-gate phase durations: **10 of 10 `--all`
  phases** (`ui-host` remains a separately measured required-host run);
- full-run budget coverage: **100% or the full gate exits nonzero**;
- changed-aware measurement coverage: **best effort with a visible warning;
  never a new reason for the test loop to fail**;
- KR4 clock start: timestamp of the first complete schema-1 `full` record;
- KR4 evidence command: `uv run python scripts/test.py --history 30`.

The Wave should review the history weekly. The feature succeeds when the first
30-day window reaches `HOLDING`; any earlier overrun or incomplete record names
the exact phase that resets the evidence claim.
