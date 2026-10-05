# Task conversation and ordinary Flows — design for the next pass

October 5, 2026. LOO-353, Product, PR #1439. Rewritten after Jack Heart's
delivery review of `e10e2add4`. The reviewed evidence and Jack's exact words
are in [demo-task-flows.md](demo-task-flows.md); open choices are in
[questions.md](questions.md); the code walkthrough of the reviewed revision is
[pr-review.html](pr-review.html). The previous design, including the retained
FlowSession architecture this one removes, is at
`6f246fda4:scratch/focus-on-your-own-work.md`.

## Outcome

One native Task conversation holds design and review, files, drafts, shells and
layout. Headless work is the ordinary `lf -b` command run in the Task's
worktree. Running a Flow leaves nothing behind beyond what `lf` logs for any
command.

## Decisions by Jack Heart

October 4 (unchanged):

- Interactive Task design and review stay in the ongoing Task conversation;
  more conversations may be opened deliberately. Headless work goes through
  ordinary `lf -b`; substantial implementation in the conversation is
  discouraged, incidental edits allowed.
- Product says **Session** for interactive and **Run** for headless
  AgentSessions; identity, history and attribution survive mode changes.
- **Waiting** is the one attention state; `--waiting` replaces `--needs-me`;
  `--interactive` stays a mode filter. Provider-specific signals and false
  positives are acceptable. Claude: binary stream-json only.
- Delete all Task-worker machinery, mutable Flow switching and automatic
  database-backed recovery. The caller owns recovery.
- Delete `lf session ready`/`complete` and the review handshake; no renamed
  replacement. Conversational feedback chooses what runs next.
- Task membership is owning Home plus resolved checkout plus explicit binds,
  excluding repository/Wave scopes. Hiding or selecting never terminates or
  approves.
- Approved design: conversation-stage workflows with operational-Flow edges;
  operational Flows hold autonomous loops and XORs only.

October 5 (delivery review):

- The cut "should have been DELETING the task worker APIs and routing more
  things through the basic (e.g. flow -b) apis." Task status presentation is
  "not the current focus of the task."
- "-b should continue to print the output and block just like it used to."
  `--mode`: "Definitely take this back."
- "running somethign from a task's worktree and passing --task should be
  equivalent ... we shouldnt get different codepaths or validations for
  either."
- "`lf task run` or whatever is allowed to do #1 and #3 and then go into the
  lf run flow": place the Task; fill defaults. On launch-time PR preparation
  (#2): "im not sure why we need 2."
- "there shouldnt be anything extra on top of what you get when you run
  `lf flow ...` in a taks worktree. we should still have good records of whats
  flows are running and at what stages just via normal logging from lf binary.
  I dont think we need the FlowSession datatype." Then: "fold it into this pr."
- `lf commit`: "should always push i guess ... or else should accept -p."

## Branch state at `6f246fda4`

Done: scheduling and its five columns; Ready/Complete, review launch and
tokens; saved Flow resume; Task-worker claims, generations, `__worker`,
`task restart`, `--retry`, managed flags, `tasks.current_invocation_id`;
`controller/`; builtin Flows without human steps (`feature` = `task-design`
then `pursue`, ends at a published PR; `code` = `pursue`; `ship-demo` gone).
`-i/-b/--tui/--ide` and `commit -p` restored and `--mode` deleted
(`8847ba5ab`). Completion, cleanup and landing wait only for live or
unresolved execution.

Still present and now wrong: `lf --task ISSUE flow start`, its tmux launcher,
`LF_TASK_FLOW_OPTIONS`, and every FlowSession structure.

## Next pass

Do these in order; each leaves the build green.

### 1. One Task entry, then ordinary run

Replace `flow start` with one thin command (working name `lf task run ISSUE
[FLOW]`; Jack left the name open). It may do exactly two things, then enter
the same code as `lf --task ISSUE run FLOW`:

- **Place:** create the worktree for an unplaced Task, apply `--stack-on`,
  restore the checkout. This is `ops::task::task_run` with `launch: false`,
  which `lf task checkout` already calls.
- **Default:** the Project's Flow when none is named; `-m` recorded as the
  Task's agent; `--reason` published as a steer.

It honors `-b`/`-i` like any run and blocks. Delete `launch_task_flow`,
`launch_existing_task`'s launch tail, `ops/run.rs::exec_task_flow`,
`TaskFlowExec`, `TASK_FLOW_OPTIONS_ENV`, the tmux session naming and the
ten-second wait.

Drop launch-time PR preparation (`task_recovery_adoption`,
`reconcile_task_pr`, `clear_task_pr_merge`, `refuse_dirty_between_prs`,
`ensure_working_pr` at `ops/task.rs:4759-4784`). Before deleting, prove with a
test that `pr publish` succeeds or fails clearly when a Flow commits in a
worktree whose PR already merged; `lf pr reconcile` and `lf pr next` remain
the owners.

### 2. One path, one set of checks

`--task X` already enters the Task worktree (`bin/lf.rs`:
`prepare_work_binding`, `CwdGuard::enter`), and a launch from the worktree
without `--task` already binds to the Task. Make validation match: whatever
check a Task launch gets, it gets from Task resolution, whichever way the Task
was named. `require_task_launch` and the planning refusal (terminal, moved or
removed Task) are currently reached only through `flow start`.
`require_autonomous_steps` applies to every Flow launch.

### 3. Remove FlowSession

Target: a Flow is a driver Exec and its child step Execs.

- **Record.** Flow name, step label, graph key, loop path and per-edge
  iteration go on the step Exec's command/argv. Delete `flow_sessions`,
  `flow_events`, the cursor, `position_version`, `pending_session_id`,
  `store/sqlite/flows.rs`, `store/flows.rs`, `flow_inventory.rs`, and the
  `FlowSession`, `FlowTurnSelection`, `FlowFilter` types.
- **Driver.** Graph and cursor live in the driver's memory; it passes each
  step to its child as arguments. Delete `__flow-step <flow> <n>` reading a
  row, `ops/flow_run.rs::driver_lock`/`driver_live`, and
  `settle_flow_step`/`record_flow_cursor`. Liveness is Exec process evidence.
  Keep same-Session decision-output repair and the engine traversal and XOR
  selection.
- **Receipts.** Publish and landing idempotency stay in PR and landing
  records. First confirm nothing reads the `flow_events` copy
  (`ops/pr_landing.rs`, `ops/task_execution.rs`, CI repair); move any reader
  to the PR/landing record.
- **Readers.** `task status`, `execution_blockers` (`ops/task/lifecycle.rs`),
  chapter "started work", `lf flow list/show --sessions` and Desktop's Flow
  graph (`TaskFlowSnapshot`, `TaskFlow.swift`, `TaskFlowView`) read Execs plus
  the authored template. A past Flow whose YAML changed is drawn from its Exec
  sequence. Remove `agent_sessions.flow_session_id`; Sessions link through
  their Exec's parent.
- **Migration.** Extend the one draft `task_flow_observations`: archive each
  Flow row's name, state and last cursor as observations on its Sessions or
  Execs, then drop both tables and the Session column. No second draft. No
  live Home conversion.
- **Docs.** Rewrite AGENTS.md "Wave Planning" and
  `docs/architecture-reference.md` where they say one started Flow is one
  FlowSession; README, `docs/`, DTO fixtures.

### 4. Callers

`git grep -lE "flow start|LF_TASK_FLOW_OPTIONS"` lists 49 files: builtin
skills (`task_operate`, `advance`, `launch-plan`, `repo_operate`,
`repo_session`, `wave_operate`, `wave_session`, `s1`, `init`),
`skills/loopflow/SKILL.md`, `docs/`, README, Desktop Start
(`RegistryQuery.swift:190`, `MacLocalWaveAgentLauncher.swift:96`) and tests.
Point them at the entry or at `lf --task X -b <flow>`. Callers that relied on
an immediate return background the command themselves; see
[questions](questions.md).

## Later slices (approved October 4, unbuilt)

Flow history in these now means Exec sequence, not captured rows.

- **Workflows.** `.lf/workflows/<name>.yaml`: conversation-stage keys with
  optional review skills, and edges with `from`, `to`, `flow`. Source guidance
  only: no Work kind, runtime row, cursor or token, never compiled into a Flow
  graph. The Task conversation reads results and launches the next edge with
  `lf -b`. Questions, holds and ambiguous replies launch nothing. Author the
  builtin kickoff/review and demo/revision stages this way.
- **Loops.** One form: a deciding node last, `loop: <target>` with optional
  `step` defaulting to `loop-or-next`, replacing `loop-decide` and
  `repeat.from` in catalog, skills, templates, docs and tests with no alias.
  Unique skill names resolve the target; one optional occurrence name
  disambiguates. Advance, Iterate or Blocked; malformed output never advances.
  Shared and overlapping return edges are preserved.
- **Task primary.** `PrimaryScope::Task(TaskId)` under the existing scope
  lock. Explicit choice wins; else the sole unfinished interactive Task
  conversation; else the most recent by `latest_interactive_session`'s ranking
  (native human input, interactive opening, creation), restricted to
  unfinished Task members. Read-only inventory creates nothing.
- **Waiting.** Rust-owned. Immediate on explicit pending input or a successful
  interactive yield with no outstanding tools; otherwise after 120 seconds
  without provider activity and zero unresolved tool calls. No event yet is
  opening/unknown; disconnection is unknown. New activity clears it.
  `session list --waiting` filters before paging. Evidence per provider:
  [harness-attention.md](harness-attention.md).
- **Desktop.** Every Flow of the Task gets the same graph/progress/output
  view; Waiting first, working Sessions in a compact group; no completion
  controls.
- **Workspace proof and scope retained from September 30:** real provider
  continuation, owning-Home remote association, cross-Task focus/input and
  process retention, same-byte symlink/read-only transitions (private Home
  copy only); twenty comparable retained-layout actions against p95 <100 ms
  with idle CPU/process counts; Project Default Flow in Wave settings; Edit
  Flow opening real source with explicit builtin customization; website
  alignment. Details: [workspace review](../docs/reviews/task-workspace.md).

Exclusions: general messaging, automatic recovery, a Run entity, Project
reset (LOO-366), completion-policy redesign (LOO-367), Claude hooks/SDK,
external-progress credit from self-hosting, Task status presentation.

## Checks for the next pass

Clear inherited `LF_*`/`LOOPFLOW_*` before Rust tests. Gate owns the full run.

| Command | Proves |
| --- | --- |
| `cargo test -p loopflow --test task_flow_launch_tests` | The Task entry places, defaults, then runs in the foreground; `flow start`, `task restart`, `--retry` are rejected; a worktree launch and a `--task` launch record identical Execs and get identical refusals. |
| `cargo test -p loopflow --test flow_tests` | Nested loops, repeated skills and XORs read back from Execs alone; a killed driver leaves dead Execs and no restart; human steps rejected. |
| `cargo test -p loopflow --test pr_tests --test land_tests` | Publish/landing idempotency without `flow_events`; publish in a merged-PR worktree. |
| `cargo test -p loopflow --test dto_fixtures` then `scripts/test_desktop.sh --no-parallel -Xswiftc -gnone` | Task and Flow DTOs agree across Rust and Swift with no FlowSession fields. |
| `uv run python scripts/materialize_rust_tests.py -- cargo test -p loopflow --lib store::` | Released frontier converts through the one draft, archiving Flow rows. |
| `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `uv run python scripts/check_architecture.py` | One implementation. |
| `git grep -E "FlowSession\|flow_sessions\|flow_events\|flow start\|LF_TASK_FLOW_OPTIONS"` | Matches only released migrations, the draft and removed-command assertions. |

Configured demo, still separate: in a private Home, `lf task run INF-123
proof -b` prints and blocks; `lf monitor` shows a looping Flow's driver, step
and iteration; a real provider step; Desktop.

Recorded at `8847ba5ab`: build, Clippy, `cli_discovery` 18, `flow_tests` 23,
`session_lifecycle_tests` 17, `task_flow_launch_tests` 1, `run` unit tests 33,
`swift build --build-tests`. PR CI ran only `scratch-clear`.
