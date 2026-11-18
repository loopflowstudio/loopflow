# LOO-974: managed Tasks on OpenCode and Pi

## Outcome

A Task step runs the same on any of the four harnesses. `lf task run` starts the
step; the step gets context and steers, runs tools, records its answer, usage
and native thread; the Graph advances. A harness that is not installed is passed
over in a list and named once when it was the only choice.

`rust/loopflow-execution/src/harness/README.md` gets one table that says so:
a row per behavior below, a column per harness, each cell the test that shows it
or the word "no" with the Task that owns the gap. The table is how this Task is
checked, and how the next gap is seen.

| behavior | shown by |
|---|---|
| a worker starts, settles, records answer, native thread and usage; the Graph advances | `a_task_step_runs_as_a_worker_on_every_harness` |
| the step's context reaches the provider | same test, read from the stand-in's input |
| a steer sent while the step runs reaches it | same test |
| a harness that is not installed is passed over, or named once | `a_step_that_cannot_launch_is_not_started_again` |
| a question with nobody there ends the turn | slice 3 |
| interrupt stops a worker; a stopped Task Session resumes | slice 4 |
| a usage limit moves to the next agent | Claude, Codex: yes. OpenCode, Pi: no (excluded) |
| an idle work session hears a steer | Claude, Codex: yes. OpenCode, Pi: no (excluded) |

## Findings that shape the design

**The "execution boundary" denies nothing.** `AgentExecutionBoundary.writable_roots`
is built in `checkout_execution_boundary` (`rust/loopflow-execution/src/agent/mod.rs:105`)
and read by no launcher. With a boundary present Claude gets
`--dangerously-skip-permissions` (`agent/mod.rs:729`), Codex gets
`danger-full-access` with no writable-roots config (`agent/mod.rs:617`; pinned by
`managed_task_scope_carries_exact_delivery_capabilities`, `agent/mod.rs:1669`), and
Pi's argv is unchanged (`harness/pi.rs:141`). The brief's "writes elsewhere are
denied" is not true of any harness today.

**The boundary is `yolo` under four other names.** The one production writer
that matters already sets `skip_permissions = true` beside it
(`rust/loopflow/src/lf/commands/run.rs:641`). `write_scope`,
`execution_boundary`, `supports_execution_boundary` and `confine` add nothing a
person can observe on Claude, Codex or Pi; on OpenCode they add the refusal.
`confine_checkout_agent` (`run.rs:456`) is the function that turns confinement
off.

**Three words each mean two things.**
- *Execution boundary*: in `docs/security.md`, the OS account, container or VM
  the person chooses; in code and `docs/config.md:981`, a list of two paths.
- *Managed*: in `docs/glossary.md:22`, a work session or worker that reports
  turns to the server; in the boundary's error and `docs/config.md`, any
  headless run in a Task checkout, which includes a batch (`lf -b <skill>`),
  a bare launch.
- *Unattended*: in `docs/config.md:968`, every headless run and its permission
  floor. Do not reuse it for the Task-checkout rule.

**OpenCode is refused by a bool, not by a missing mechanism.**
`supports_execution_boundary: false` (`harness/opencode.rs:1393`) trips
`checkout_execution_boundary`, called for every headless run in a Task checkout
(`run.rs:635`), batch resume (`agent/session.rs:132`) and Task status
(`ops/task.rs:2299`). OpenCode's `skip_permissions` already writes allow-all
permissions (`opencode.rs:1277`).

**Worktree scope without a boundary is production-dead.** Both production
writers set `write_scope = Worktree` together with a boundary (`run.rs:636`,
`agent/session.rs:127`). The provider-only branches (Claude strict sandbox
settings `agent/mod.rs:735`, Codex forced `workspace-write` `agent/mod.rs:611`,
OpenCode `external_directory` deny `opencode.rs:1272`, Pi's `ensure!`
`pi.rs:142`) are reached only by `launch_matrix.rs:159` and a replay fixture
(`rust/loopflow/src/lf/commands/replay.rs:232`).

**The Task-checkout rule has two owners.** `run.rs:523,635` decides it from the
Task input; `agent/session.rs:101-137` decides it again for batch resume from
the stored request and the Session's cwd, and sets no `skip_permissions`.

**Three attempts come from one blind loop.** `TASK_RUN_ATTEMPTS = 3`
(`rust/loopflow/src/lf/commands/graph.rs:147`, loop 209-307) retries any Graph
child that exits 1. It stops early only for an interrupt, retained edits or a
limit row.

**A missing CLI is already refused before launch, with the hint.**
`run.rs:751` returns `missing_agent_message` before the step's runtime begins,
so no AgentProcess exists. The loop then repeats it twice. "Is this harness
installed" is asked in five places (`run.rs:751`, `replay.rs:58`,
`ops/human_session/primary.rs:148`, `harness/native_session.rs:647,682`,
`config.rs:173`).

**One place already moves a step to its next agent.** `_run_with_retries`
(`agent/mod.rs:1045`) walks `_fallthrough_rungs` (`agent/mod.rs:1171`) and
records each move with its reason (`_record_agent_move`). Only a Graph step
moves (`wait_for_capacity`). It consults neither capability nor installed
binary, and `_climb` (`agent/mod.rs:1207`) lands on OpenCode unrefused today.

**Graphs have steps that start no provider.** `cmd: sync` (`build.yaml`) and
`cmd: land --wait-and-fix` (`deliver.yaml`) are steps of the `code` Graph. A
rule keyed on "the failed step started no provider" would stop retrying them.

**`Failed{resumable:false}` has a reader and no writer.**
`task_event_process_refusal` (`ops/task.rs:2316`) matches it; nothing in
production writes it. A refusal stored there would outlive its cause: install
the CLI and the Task still refuses. This plan does not use it.

**No test runs a worker on OpenCode or Pi.** Every `task run` integration test
uses Claude or Codex. The OpenCode and Pi Graph tests use `graph … --batch`
(`LF_GRAPH_STEP=bare`, `graph.rs:851`), which is a batch. The launch matrix never
sets a boundary, so its OpenCode Worker cell passes by skipping the refusal.

**A headless OpenCode question hangs.** A worker does not answer `form.created`
(`an_opencode_question_waits_until_it_is_answered`, `harness/fact_writer.rs:634`);
Pi fails the turn with "open this Session in Pi to answer it" (`harness/pi.rs:323`).

**Seen on this Machine, 2026-11-18:** OpenCode 2.2.1 lists
`opencode/gemini-4-carbon`; Pi 1.3.0 lists Anthropic models. `lf task status
LOO-974` shows an interactive OpenCode work session for this Task as active, so
an OpenCode Task conversation starts today; only headless runs are refused.

**Open PRs:** #1974, #1976 and #1972 touch `agent/mod.rs`, the harness files and
`ops/task.rs`, but none changes a line naming `execution_boundary`, `write_scope`,
`supports_execution_boundary` or `task_configuration_refusal` (grepped their
diffs against the merge base, 2026-11-18). #1946 touches neither.

## Design

Three rules, each with one owner.

**1. A headless run in its Task's checkout runs as `yolo`.** That is the whole
permission rule, in the word the docs and the config already use.
`AgentWriteScope`, `AgentExecutionBoundary`, `supports_execution_boundary` and
`checkout_execution_boundary` go. One function in `loopflow-execution` answers
"is this a headless run in its Task's checkout" and both `run.rs` and batch
resume call it; `confine_checkout_agent` goes with its name. With no capability
to lack, no harness can mismatch.

**2. A Graph step passes over an agent whose CLI is not installed, and says so.**
`_fallthrough_rungs` drops rungs whose CLI is missing and records each as a move
with the reason "not installed", the same record a limit move writes. When no
rung is left the step fails with the first choice's install hint. The check at
`run.rs:751` moves behind the rung choice so a Graph step asks once. `Config`
stays free of Machine probes: no filter in `agent_order` or `resolve_agents`. A
chat or batch whose choice is missing fails with the hint as it does now; only a
Graph step moves, as with limits.

**3. `lf task run` starts a failed Graph again only when there was work to
retry.** A skill step that failed with no AgentProcess under its Process was
refused by lf, and the second attempt would be refused the same way: the run
stops after one attempt with the step's error. A `cmd:` step keeps its three
attempts. The rule is read from the store and the Graph's step; no failure type
and no stored refusal is added.

Made for review, with what was passed over:

- *Passed over: flip OpenCode's bool to `true`.* Leaves a capability all four
  harnesses have and a descriptor nobody reads.
- *Passed over: make OpenCode enforce the roots* (`external_directory` deny plus
  the Git and `~/.lf` roots). It would be the only harness that denies, at tool
  level only, and Task steps would fail there on reads Claude and Codex allow.
- *Passed over: a real OS sandbox for all four.* A new feature that reverses
  `docs/security.md`; see `scratch/questions.md`.
- *Passed over: filter the agent order in `Config`.* A sixth place that asks
  about installed CLIs, a silent move, and chats would start moving too.
- *Passed over: stop whenever the failed step started no provider.* Stops
  retries of `cmd: sync` and `cmd: land --wait-and-fix`.
- *Passed over: write `Failed{resumable:false}` on a launch refusal.* The Task
  would keep refusing after the CLI is installed.
- *Passed over: idle steer wake for OpenCode and Pi in this Task.* It is server
  work (`rust/loopflow/src/server/README.md`, "Waking an idle conversation"),
  not launch work, and steers still arrive at the next turn. Its row in the
  table reads "no".

## Acceptance at gate

```bash
cargo test -p loopflow --lib task_graph_launch_tests::a_task_step_runs_as_a_worker_on_every_harness
cargo test -p loopflow --lib task_graph_launch_tests::a_step_that_cannot_launch_is_not_started_again
cargo test -p loopflow --lib task_graph_launch_tests::a_failed_command_step_is_started_again
cargo test -p loopflow-execution --lib harness::launch_matrix
```

The first runs `task run` through `implement` with the Claude, Codex, OpenCode
and Pi stand-ins as workers (not `--batch`). Per harness it reads back: the
context the stand-in received, a steer sent mid-step, one settled step, an
answer, recorded usage, a recorded native thread and the Graph at its next step.
The second pins an uninstalled harness and expects one attempt and the install
hint; with a second installed agent in the order it expects the step to run
there and the move to be recorded. The third fails a `cmd:` step and expects
another attempt (reuse an existing retry test if one already covers a `cmd:`
step).

Run by a person or at demo, on a throwaway Task with the branch `lf`:

```bash
lf task run <ID> -a opencode:opencode/gemini-4-carbon    # implement settles, Graph advances
lf task run <ID> -a pi:<configured model>
```

## Surfaces

- `lf task run`, `lf ship`, `lf -b <skill>` in a Task checkout, batch resume.
- `AgentProcessRequest` (`session_record.rs:905`) loses `write_scope` and
  `execution_boundary`. It has no `deny_unknown_fields`, so stored captures
  that carry them still load for `lf replay`.
- Task status: `task_configuration_refusal` shrinks to configuration errors and
  loses its unused `skill` parameter.
- Docs: `docs/config.md:965-987` states rule 1 in one sentence and drops
  "managed", "execution boundary" and "provider-only worktree scope" (also
  fixes `permission` → `permissions`); `docs/security.md` keeps "execution
  boundary" for the OS environment only; `harness/README.md` gets the table and
  loses the capability from its row.

## Docs written first

Written as intended behavior, before the code; implement reads these and makes
them true, or changes them in the same slice.

- `docs/config.md`: Yolo (rule 1, `permissions`), Agent (`lf task run -a
  opencode:…` / `pi:…`), "Keep going when a provider runs out" (not-installed
  move, OpenCode and Pi unclassified), Pi (boundary sentence gone).
- `docs/security.md`: a headless run in its Task's checkout is `yolo`, unconfined.
- `docs/orchestration.md`: no second attempt when no agent started; a headless
  question fails the step.
- `docs/subscriptions.md`: two rows in the refusal table.
- `docs/troubleshooting.md`: "Agent not found" for a Task step.
- `rust/loopflow-execution/src/harness/README.md`: the parity table;
  "execution boundary" out of the capabilities row.

The docs commit the implementation to three things the plan had left open:
- the move line in `lf task status`: `implement went on under codex after:
  'opencode' is not installed` (the limit move's format, reason =
  `missing_agent_message`'s first sentence);
- test names for slices 3 and 4:
  `a_question_with_nobody_there_ends_the_step`,
  `a_worker_stops_on_interrupt_and_its_session_resumes`;
- a `harness::launch_matrix` cell asserting a worker in a Task checkout is `yolo`.

`documented_commands::` runs at gate (new examples: `lf task run LOO-12 -a …`,
`lf steer LOO-12 "…"`).

## Absent and error states

- Harness not installed, another agent in the order: the step runs there; the
  move is recorded with its reason.
- Harness not installed, pinned or last: one failed attempt, install hint as the
  Task's reason.
- Skill step fails before any provider starts: no second attempt; `lf task run`
  again is the retry.
- `cmd:` step fails: three attempts, as now.
- A worker's question with nobody there, on any harness: the turn fails and the
  reason carries the question and how to answer it, as Pi does now. OpenCode's
  worker does this on `form.created` instead of waiting.

## Exclusions

- Usage-limit and transient-failure classification for OpenCode and Pi
  (`classifies_failures: false`, `agent/mod.rs:1062`): a failed OpenCode or Pi
  step gets no fallback and no wait. Needs real failure payloads; OpenCode's are
  LOO-518.
- Idle steer wake for OpenCode and Pi work sessions. Needs its own Task:
  OpenCode has a private server to post to; Pi's TUI has only its extension.
- Native output schemas and turn limits; Graph output validation already covers
  route steps on both.
- Any write denial. Behavior on Claude, Codex and Pi is unchanged.
- `Failed{resumable}`: dead on the false side, left alone here.

## What goes

- `AgentWriteScope`, `AgentExecutionBoundary`, `checkout_execution_boundary`,
  `Capabilities::supports_execution_boundary`, `confine_checkout_agent`.
- Provider-only worktree branches: `agent/mod.rs:611-616`, `735-752`,
  `provider_writable_roots`'s boundary test, `opencode.rs:1272-1276`,
  `pi.rs:142`, `codex.rs:673`'s scope test.
- Tests: `managed_task_scope_carries_exact_delivery_capabilities`,
  `execution_boundary_resolves_linked_git_and_control_roots`, the matrix's Pi
  special case and its `external_directory` assertion.
- Survives: Claude and Codex worker argv and params, apart from `--add-dir` of
  the main repository now appearing on worktree steps as it does on every other
  headless run.

## Slices

1. **Admit OpenCode and see what else breaks.** Delete the capability and the
   boundary types, give rule 1 its one owner, add the four-harness worker test
   and the README table with honest cells. Then run the real `implement` step on
   `opencode/gemini-4-carbon` and a Pi model. This is the slice that can prove
   the plan wrong: it shows whether the plugin's turn reports, turn completion,
   usage, steers and Graph advancement hold for a worker, which no test has
   exercised. A cell that fails is fixed here or written as "no" with its cause.
2. **A step that cannot launch.** Rules 2 and 3 and their three tests.
3. **A worker's question ends the turn** on OpenCode, matching Pi.
4. **Cancel and resume, on both.** Interrupt a running worker and resume a
   stopped Task Session per harness; OpenCode's interrupt has no test today.

## Done when

`lf task run` on a Task pinned to OpenCode Gemini 4 Carbon, and on one pinned to
a Pi model, settles `implement` and moves to the next step; pinning a harness
that is not installed prints the install hint after one attempt; the README
table has a test or a named gap in every cell.
