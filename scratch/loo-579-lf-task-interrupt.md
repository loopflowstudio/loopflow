# LOO-579: `lf task interrupt` stops a running Task Graph

Status: implemented (2026-10-24), including the concept review's receipt repair.
No product decision is open.

## Problem

`lf task interrupt <issue>` reports "interrupted", but a running Task Graph
carries on. During LOO-575 (Oct 24) the Graph advanced into `implement` and its
processes had to be killed by hand (incident LOO-578, analysis section D).

## What happened in LOO-575 (answers the brief's open question)

From `~/.lf/loopflow.db` and `lf graph show --processes`:

| t (unix) | event |
|---|---|
| 1792900263 | attempt 1 (`8985247f`) `sync` fails, exit 1 → `lf task run` retries |
| 1792900267 | attempt 2 (`d6c88553`) starts `kickoff` (step `7f5d53b9`) |
| **1792900355** | **task_events 70149 `interrupt`** |
| 1792900405 | `kickoff` exits **0** (50 s after the interrupt) |
| 1792900406 | `implement` starts; never finishes (killed by hand) |

So an agent turn was live when the interrupt was posted, and it never saw it.
The cause is not the headless exit code. Graph steps run with
`Cli::step_args()`, which omits `--task` by design (the step resolves its Task
from the checkout). `prepare_task_input` only builds a `TaskInput` from an
explicit `cli.task`, so **no Graph step ever polls `observe_interrupt`**. The
step's argv confirms it: `--batch --skill-input … skill kickoff`.

The brief's other gaps hold too. `automation_enabled` is read only by the
scheduler. Nothing checks for an interrupt between steps, during an op step, or
before `run_for_task` retries. And any reader that did would take its cursor
when the step starts, which absorbs an interrupt posted before that.

## Demo

```
$ lf -b task run LOO-x autonomous &      # Graph at implement
$ lf task interrupt LOO-x
LOO-x: stopped Graph autonomous at implement (step Process …)
# The Task run exits 130, `implement` ends as interrupted, no retry, and no later step starts.
$ lf task interrupt LOO-x
LOO-x has no live run; nothing to stop. Scheduled automation stays off.
```

## Chosen approach: interrupt is a control on the Task run, owned by the Graph process

One reader per Task run, with one cursor taken when the Task run starts.

1. **Cursor at Task-run start.** `run_for_task` reads
   `latest_interrupt_id(Task)` once, before the first attempt. It passes the
   value to every Graph attempt as `LF_TASK_INTERRUPT_CURSOR`, following the
   `LF_GRAPH_ID` precedent. A Graph started without it (direct `lf graph --task`)
   reads its own cursor when it starts. Taskless Graphs have no cursor.
2. **Graph process checks before every spawn.** At the top of
   `RunningGraph::spawn`, which covers skills, routers, op commands and
   correction turns: if `latest_interrupt_id > cursor`, the step is not started
   and the result is `StepExit::Interrupted`.
3. **Graph process watches while a step runs.** The `spawn` select loop polls
   the interrupt every 1 s (it already ticks every 10 ms to record the step).
   - In an **agent step** (`run_skill`: skills, routers, correction turns) the
     Graph process sends SIGTERM to the step child with `platform::kill_process`.
     The step's ctrlc hooks finish its Session capture as interrupted, kill the
     provider, and exit 130.
   - In an **op step** (`run_command`) it lets the command finish, then stops
     before the next step whatever the op's result. An op is never killed
     midway: LOO-578 shows that killing `sync` midway is what strands work.
4. **One interrupted outcome.** `StepExit::Interrupted` → `SkillOutcome::Interrupted`
   → `GraphOutcome::Interrupted` → `report_outcome` returns `CommandExit(130)`
   with "Graph <name> interrupted before/at <step>". Exit 3 (`GraphHeld`) stays
   reserved for held Graphs.
5. **Exit 130 throughout.** A headless agent turn that ends `Lifecycle::Interrupted`
   exits 130 (`agent/mod.rs` exit-code mapping; today it exits 1), so `spawn`
   maps 130 → Interrupted with no turn-event inspection. A step child killed by
   signal also exits 130.
6. **No retry after an interrupt.** `run_for_task` already passes through any
   exit other than 1 without retrying. It also checks `latest_interrupt_id > cursor`
   after every attempt. When true, it prints "Task run interrupted; not starting
   Graph again" and exits 130, even if the attempt failed with 1 for an
   unrelated reason.
7. **Truthful receipt.** `task_interrupt` decides liveness before it appends
   (`live_execution` in `ops/task.rs`):
   - live = the Task's latest Graph process is not Dead and its execution is
     Running, Stalled, Starting or Unknown (`execution_of_latest_graph`); otherwise any open, not-dead Process in the
     Task's work outside the caller's lineage (`task_open_processes` +
     `unresolved_processes`, the set `execution_blockers` already uses). That
     covers a Task run between attempts and a direct Task Session with one
     detector, without parsing argv.
   - live → append the Interrupt event. `WorkControlReceipt::Interrupt { work, target }`,
     where `target` reads like "Graph autonomous at implement" or
     "Process <id> running `lf task run …`".
   - not live → append nothing, return `WorkControlReceipt::NoLiveRun { work }`,
     print "has no live run; nothing to stop" and exit 0.
   - Both paths still call `set_task_automation(false)`, which is unchanged.
   - Unknown process evidence counts as live (the interrupt is cheap and safe),
     and the receipt says liveness is unknown.
   - Known over-report: an unrelated interactive Session in the checkout also
     counts as live (see `scratch/questions.md`).

`TaskInput`/`observe_interrupt` stay for commands run with `--task` (direct
Task Sessions), unchanged.

## Alternatives rejected

- **Pass `--task` into Graph steps** so each builds a `TaskInput`. That gives two
  observers per step, each with a cursor taken when its step starts (the same
  absorption bug), and it changes every step's seed message and stdin attachment.
- **`run_for_task` polls and kills the Graph attempt.** That puts a second owner
  on the control and kills op steps midway, which is the LOO-578 damage.
- **Reuse `automation_enabled` as a stop flag.** It means "don't schedule". A
  durable boolean also outlives the run it was meant for, so the next
  `lf task run` would need a reset, a second writer.
- **Derive the Graph's cursor from the parent Process's `started_at`.** Event
  timestamps have one-second resolution. The env value is exact and boring.

## Risks found

- SIGTERM to a headless `lf skill` goes through `exit_on_interrupt`: hooks run
  (`session_record` finishes capture "interrupted", transport kills the
  provider), then `exit(130)`. Confirmed in `agent/mod.rs` and the `session_record`
  cleanup registration. No new signal plumbing.
- Claude's `interrupt()` already kills the provider process. Signalling the step
  loses nothing compared with `harness.interrupt()`.
- An op step that runs agents itself (`pr land --wait-and-fix`) runs to
  completion before the Graph stops. This is an accepted exclusion.
- `LF_TASK_INTERRUPT_CURSOR` must not reach step children: `spawn` calls
  `env_remove` on it, so nested `lf` calls from agent shells never inherit a
  stale cursor.

## Scope

**User-visible outcome.** Whoever runs `lf task interrupt` on a Task with a live
Graph or Task run sees it stop: an agent step ends now, an op step finishes and
nothing follows, and the Task run exits 130 without retrying. If nothing is
live, they are told so.

**Source of truth.** `task_events` Interrupt rows (unchanged). The Task-run
cursor is in memory and passed through the env. Liveness comes from the Process
evidence that already backs `TaskExecutionSnapshot`.

**Affected surfaces.** `lf task interrupt` (text and `--json` receipt; there is
no Swift or Python consumer and no DTO fixture), `lf task run`, `lf graph`
exit codes (130 is new for interrupted Graphs; 3 still means held),
`repo_operate.md`, `docs/lf.md`, `docs/troubleshooting.md`,
`docs/lf-reference.md` (regenerate) and the `Interrupt` help text.

**Exclusions.** Interactive Workflow nodes; killing op steps; Wave/Project
interrupts (`WorkRef::Wave` returns 0 today); changing `automation_enabled`
semantics.

## Delete — do not maintain

- `StepExit::Stopped` covering both 3 and 130: split into `Held` (3) and `Interrupted` (130).
- The `spawn` turn-events scan for `turn_completed`/`interrupted` (graph.rs,
  `_ =>` arm). It is dead once interrupted headless turns exit 130. Keep the
  provider `error` scan.
- The `task_interrupt` doc comment "With no live run it is inert", the
  `Interrupt` help "Interrupt the active provider turn", and the troubleshooting
  sentence "ends the active boundary so the next one re-reads direction".
- `repo_operate.md:177`: the sentence is missing its verb. Make it "`lf task interrupt <issue>` stops its live execution".

## Forbidden outcomes

A second stop store or flag; `--task` threaded into Graph steps; any path that
kills an op step midway; `run_for_task` signalling the Graph; a receipt that
says "interrupted" when nothing live was found.

## Slices

One indivisible change, implemented: Graph-side control, `run_for_task`
cursor and no-retry check, truthful receipt, and docs.

Implementation notes (2026-10-24):
- The receipt's text comes from `WorkControlReceipt::summary`; `label`/`action` are gone.
- A headless `lf skill` whose turn ends interrupted exits 130 (`run.rs`).
- `LF_TASK_INTERRUPT_CURSOR` is set only for a Graph child of `lf task run`.

## Receipt repair (concept review, 2026-10-24) — implemented

The brief says `lf task interrupt` "reports success only for something it
actually stops". `live_execution` names the Graph only while the Graph's own
process is Live or Unknown, because a step whose Graph process is dead has no
poller. Otherwise an open Process gets "interrupt recorded for Process <id>
running `…`; a Task run or a `--task` Session stops on it", never "stopped". The
idle text is "Scheduled automation is off." There is still one `Interrupt { work,
target }` variant, and no JSON consumer branches on the distinction. Covered by
`interrupting_a_task_with_only_an_open_process_records_it_without_claiming_a_stop`.

Second concept review (2026-10-24, against 8e876baff3): no change. During an
op step the receipt still reads "stopped Graph X at <op>" while the op finishes;
the accepted Demo wording and point 3 cover that (the Graph does stop, and
nothing follows), so it stays.

## Done when (gate, headless)

From an lf-launched shell, with inherited `LF_*` unset and `< /dev/null`:

- `cargo test -p loopflow --lib task_graph_launch < /dev/null` (the file is
  compiled into the library from `lib.rs`) includes two new tests in
  `tests/linear/task_graph_launch_tests.rs`:
  - **op-step interrupt**: Graph `interrupting.yaml` =
    `- cmd: task interrupt INF-123` then `- cmd: task sync --plan`. Run with
    `lf -b task run INF-123 <edge>`. Expect exit 130, `attempts(...)` of length 1,
    exactly one step recorded in the Graph process (the second never spawned),
    no "starting it again" in stderr, and receipt target naming the Graph.
  - **idle receipt**: `lf task interrupt INF-123 --json` on an idle Task returns
    `kind: no_live_run`, and the number of `task_events` interrupt rows does not change.
- A unit test in `lf/commands/graph.rs`: `RunningGraph` with a Task runs an
  agent step whose stand-in `LF_BIN` sleeps. An interrupt row is appended
  mid-step. Expect the step child to exit 130, `GraphOutcome::Interrupted`, and
  no second step.
- `cargo clippy --all-targets -- -D warnings`; `cargo fmt --check`.
- Regenerate the reference with `UPDATE_CLI_REFERENCE=1 cargo test -p loopflow --test integration reference_matches_the_compiled_cli < /dev/null`.

Check result (kickoff + compress, on #1676 at 72598db02f): `cargo test --lib interrupt` 17 ok, `--lib task_graph_launch` 34 ok, `--test integration task_github_cache` 3 ok, reference test ok; clippy/fmt clean. Full suites deferred to gate.
