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
| the step's context reaches the provider | `a_task_step_runs_as_a_worker_on_every_harness` |
| a steer sent while the step runs reaches it | same test |
| the step's usage is recorded | same test, all four |
| a harness that is not installed is passed over, or named once | `a_step_that_cannot_launch_is_not_started_again` |
| a question with nobody there ends the turn and stops the run after one attempt | OpenCode, Pi: `a_question_with_nobody_there_ends_the_step`. Claude, Codex: no, not read |
| interrupt stops a worker with its turn recorded interrupted; the next run starts the step again | `a_worker_stops_on_interrupt_and_the_next_run_starts_its_step_again`, all four, using this test filter |
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

**Before slice 2, three attempts came from one blind loop** (`TASK_RUN_ATTEMPTS`,
`rust/loopflow/src/lf/commands/graph.rs:146`) and a missing CLI was refused at
`run.rs` before any AgentProcess, then repeated twice. "Is this harness
installed" is still asked in several places (`replay.rs`,
`ops/human_session/primary.rs`, `harness/native_session.rs`, `config.rs`).

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

**Before slices 3 and 5, a headless OpenCode question hung** on the server's
`form.created`. `opencode run --auto` does not hang: see slice 3.

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
`run.rs` moves behind the rung choice so a Graph step asks once. `Config`
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
cargo test -p loopflow --lib task_graph_launch_tests::a_question_with_nobody_there_ends_the_step
cargo test -p loopflow --lib task_graph_launch_tests::a_worker_stops_on_interrupt_and_the_next_run_starts_its_step_again
cargo test -p loopflow-execution --lib harness::opencode_decode::
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
  `a_worker_stops_on_interrupt_and_the_next_run_starts_its_step_again_on_*`
  (two tests, two harnesses each);
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
- A worker's question with nobody there, on OpenCode and Pi: the turn fails,
  the run stops after that one attempt, and the stop carries the question and
  the two commands that answer it (slice 7). Claude and Codex: not read.

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

1. **Admit OpenCode. Built.** The capability and boundary types are gone;
   `agent::headless_task_checkout` is the one owner of rule 1, called by
   `run.rs` and batch resume. `a_task_step_runs_as_a_worker_on_every_harness`
   runs `implement` as a worker on all four stand-ins and reads back one
   settled step, the Graph at `review`, a recorded native thread and the final
   answer through `lf history show --final`.
   - Context, a mid-step steer and usage were added in slice 6.
   - Not run: `lf task run` on a real Task with real OpenCode or Pi. A worker
     needs the Machine server, and the branch `lf` was only run against an
     isolated `LF_HOME`. Demo owns it.
2. **A step that cannot launch. Built.** `agent::start_on_installed_agent`
   puts a headless launch on the first installed agent of its order before the
   conversation opens (`run.rs`), records a passed-over first choice as a move,
   and takes missing agents out of the fallback order. `lf task run` stops
   after one attempt when `graph_failed_before_its_agent` holds: the failed
   step is a skill step whose Process captured no Session input.
   `a_step_that_cannot_launch_is_not_started_again` covers the pin and the
   move; `a_failed_command_step_is_started_again` is the existing `cmd:` retry
   test under its plan name.
   - "No AgentProcess under its Process" is read as "no captured input"
     (`process_inputs`, already indexed by Process). A launch that captured and
     then failed to spawn still gets three attempts.
   - The stop reads `the run of <graph> stopped before any agent started,
     and lf task run <ID> starts it again there: <cause>`, and ends an
     Auto-Ship approval like the other stops.
   - A missing agent later in the order is dropped without a record: one
     recovery row per step holds the move that happened, not ones that might.
   - Both tests passed locally, 2026-11-18 (`cargo test -p loopflow --lib`, 104s).
3. **A worker's question ends the turn. Built for OpenCode and Pi.**
   - Seen on OpenCode 2.2.1 (`harness/testdata/opencode_run_question.jsonl`):
     `run --auto` does not wait. It prints the `question` tool call as a
     `tool_use` line with `state.status: "error"` and the error "This
     non-interactive run cannot ask the user questions, so the question was
     cancelled. Continue without an answer; make reasonable assumptions and
     state them.", the model answers anyway, and the run exits 0.
   - `opencode_decode::Turn` fails the turn at that line with
     `OpenCode asked a question with nobody there to answer it: <question>`,
     and the driver's reader kills the run so it does no work on an assumed
     answer. Pi's dialog failure uses the same sentence
     (`common::unanswered_question`) with the dialog's `title` and `message`.
   - `a_question_with_nobody_there_ends_the_step` runs `implement` as a worker
     on the OpenCode and Pi stand-ins and reads the reason from `lf task run`
     and `lf task status`. Passed here 2026-11-18, 72s alone
     (`a_task_step_runs_as_a_worker_on_every_harness` 60s alone). Run together
     straight after a build the pair took 329s, cause not looked into; nextest
     kills a test at 300s, so watch both in CI.
   - Not built: Claude and Codex (no decoder fails a turn on a question, no
     trace recorded; the table says "not shown"), and stopping the two retries
     that ask the same question again. `scratch/questions.md` 18 and 19.
4. **Interrupt, then run again. Built for OpenCode and Pi.**
   - The interrupt test (renamed and widened in slice 7) holds `implement`
     mid-turn on each stand-in (`hold-turn` in the home; the stand-in writes
     `holding` once its turn is under way), runs `lf task interrupt`, and
     reads: exit 130, one attempt, the worker's turn `interrupted`, no answer.
     Then `lf task run` again settles the step and the Graph stands at `review`.
   - What it found: the Graph stops a worker with SIGTERM within a second,
     before the worker's own five-second interrupt poll calls
     `Driver::interrupt`. The provider then died under a driver that had not
     asked for it, and the turn was recorded `failed` ("OpenCode exited before
     finishing the turn", "pi_disconnected: Pi RPC stream closed").
     `agent::interrupted()` now says a signal is ending the lf process, and
     both readers settle the open turn `Interrupted`.
   - The plan was wrong about resume. "Resume the Task Session and see the
     same native thread continue" cannot be built for a worker: its Session is
     a Run, and `run_refusal` refuses to continue a Run on every harness. The
     next `lf task run` starts the step in a new conversation, and the test
     asserts that. A conversation continuing its native thread is shown below
     the Task level: `each_turn_is_one_run_that_continues_the_named_conversation`
     (OpenCode), `pi_headless_resumes_a_conversation_recorded_by_its_id` (Pi),
     `terminal_first_launch_and_failed_startup_reopen_the_same_conversation`.
   - The OpenCode stand-in now holds after its `step_start` line, as the real
     CLI prints one before working; `transport_tests`' three `hold-turn` tests
     still pass.
   - Timing, 2026-11-18: 39s on a second run, 120s and 175s on the first run
     after a build; the two timed harness bodies took 12s each either way. The
     329s seen in slice 3 was also a first run after a build. What the first
     run spends is not known.
   - Not built: Claude and Codex in this test (`scratch/questions.md` 22).
5. **OpenCode launches match Claude and Codex. Built.**
   - Headless (`harness/opencode.rs` `Driver`): each turn is one
     `opencode run --standalone --format json --auto [--model m] --session ses_…`
     process with the prompt on stdin. `opencode_decode::Turn` decodes its
     lines; the process exit settles the turn. A second turn is another run on
     the same `--session`, as a new AgentProcess of the Session.
   - Terminal (`Harness::open`): `opencode --standalone --session ses_…
     [--prompt p]`, exec'd like Claude. Work sessions take the same path: no
     server is kept, because nothing wakes an idle OpenCode TUI today (its row
     in the table is "no").
   - lf names the conversation. `Harness::thread_name` turns the assigned UUID
     into `ses_<uuid>`; OpenCode's naming is `Assigned` for every launch kind.
   - Deleted: the `opencode serve` child, Basic auth, the `reqwest` client and
     SSE reader, `open_agent_session` and its `/move` and `/model` calls,
     `native_session::Companion`, `OpenCodeDecoder` and its snapshot tests,
     `opencode_question.jsonl`, the permission auto-reply.
   - The stand-in is `rust/loopflow-execution/tests/support/opencode_cli.py`.
   - Steers to a running headless turn are no longer posted by lf
     (`send_current` is `NotSteerable`); the plugin's between-tool check
     delivers them, and one that arrives after the last tool seeds the next turn.

6. **The worker test reads back context, a steer and usage. Built.**
   - `a_task_step_runs_as_a_worker_on_every_harness` leaves
     `tests/support/tool_turn.py` in the home. Each stand-in that finds it
     runs, mid-turn, the two `lf __context-block` calls its harness makes
     around a tool: Claude from `--settings`, Codex from `thread/start`'s
     `config.hooks`, OpenCode from the plugin's `spec` and `delivery`, Pi from
     the extension's `spec`. The test runs `lf steer INF-123` while the tool
     holds, then reads the block (this worker's Session and Task) and the steer.
   - What it found: nothing in production calls `Driver::send_current`. Every
     harness hears a mid-turn steer only through the hook after its next tool.
     Slice 5's note that OpenCode's `send_current` is `NotSteerable` changed
     nothing a person sees.
   - What it found: a Pi worker's usage is not in the Task's history. lf reads
     usage from the provider's transcript and writes it to the turn the
     transcript names. Pi's session file names the turn by its entry id
     (`rpc-user-1`); lf recorded the same turn under its own request id, with
     the Task, Graph and step on it. `history.turns` ends with two rows for one
     turn, and `lf history usage --task` counts the one without usage. OpenCode
     avoids this by naming the turn after its store's `user` row. The README
     cell says no; the test skips the usage assertion for Pi.
   - The Claude and Codex stand-ins reported no usage. `keep_claude_turn` now
     dates its rows and gives the answer a `usage` (lf skips a turn whose
     transcript has no timestamp); `codex_rollout.py` writes a `token_count`
     with a running total. Every fixture using them now records 20 input and 5
     output tokens a turn.
   - The stand-in shows lf's side of the hook. The OpenCode plugin and the Pi
     extension run under Node in `harness::context::tests::`. Nothing has run
     either on a real CLI as a worker; demo owns that.
   - Passed here 2026-11-18: the worker test alone 42s (115s on the first run
     after a build); with the question and interrupt tests under nextest, 104s
     and 106s each. Nextest's limit is 300s.

7. **The parity table meets its own rule. Built.**
   - `a_worker_stops_on_interrupt_and_the_next_run_starts_its_step_again`
     filters two tests covering all four harnesses. Each checks exit 130, one
     attempt, an interrupted turn without an answer, and a new conversation
     that reaches `review` on the next run. Claude settles its pending turn
     on signal-driven EOF. Codex records its owned headless app-server's
     pending turn in the interrupt cleanup before lf exits.
   - The Pi transcript reader attaches usage and the final answer to the
     recorded turn when its timestamps cover the transcript entry. An exact
     entry id takes precedence. The worker test reads Pi's usage from
     `lf history usage --task`; the transcript test checks that it creates no
     second turn. The history query uses the session/thread prefix of the
     unique index (checked with 100,000 synthetic turns).
   - OpenCode and Pi emit `unanswered_question`. The Task run stops after
     one attempt and prints the question, `lf steer`, and `lf task run`.
     The question test reads the answer back in the fresh conversation
     and checks that the Graph reaches review. Claude and Codex question
     handling remains a named gap in the parity table.
   - Passed here 2026-11-18 under nextest, the four worker tests together on
     the first run after a build: 254s, 254s, 257s, 268s. Nextest kills a test
     at 300s; question 24. Clippy and `documented_commands::` clean.

Compress (2026-11-18): `tests/e2e/capture_history.py` read the deleted
`opencode_server.py`; v0.13.3 now gets its release's stand-in via
`git show v0.13.3:…` and the candidate gets `opencode_cli.py`. Not run here
(needs the v0.13.3 archive); gate or a person with the archive runs it.
Clippy clean; `harness::pi` and `agent::session` tests pass.

Concept review after slice 7 (2026-11-18, `scratch/concept-review.html`)
changed nothing in the plan; slice 8 later replaced its one leftover (the
question read from error prose).

QA round 2 (2026-11-18, `scratch/qa.html` scenarios 8-14): real Pi usage
is one turn row with usage; a real OpenCode question stops after one attempt
and steer-then-run answers it; real Claude and Codex workers record an
interrupted turn, and Claude's rerun settles (Codex's hit the account's usage
limit). Fixed in QA: the exit line no longer glues "." onto a reason ending in
"?"; `harness::asked_question` reads the question for both the stop and
`lf task status`, which now gives the steer-then-run advice instead of "launch
fresh work"; the limit-waiting line trims the provider's period.

8. **Human-review pass (`1a 2b 3b`, 2026-11-19). Built.**
   - `Driver::send_current` and `SendCurrentOutcome` are gone, with what
     existed only for them: Codex's correlated-request waiters
     (`PendingReply`, retired ids, `classify_steer_rejection`), Pi's steer
     receipts and its `pi_rpc.py` steer branch, and Claude's coalescing
     `pending_results` counter (one result now ends one turn).
   - OpenCode and Pi keep `{code, message}` on the failed turn's completion
     (`loopflow_storage::session::turn_error`), so `history.turns.error`
     carries `unanswered_question`. `SqliteStore::unanswered_question(step)`
     reads it by the step's process (`turns_process` index);
     `graph_unanswered_question(run)` takes the run's last failed step.
     `lf task run` and `lf task status` share `question_advice`.
     `harness::asked_question` and the prose split are deleted. The step
     error text is unchanged: an object error with `code` and `message` reads
     as `code: message`.
   - Parity table names Task 48edb9, 98edc6 and 540073 (not yet synced to
     Linear on 2026-11-19).
   - Passed here 2026-11-19: `a_question_with_nobody_there_ends_the_step`
     (104s), harness, storage turns/session_events/graph_inventory unit
     rows, `documented_commands::`, clippy.

Compress (2026-11-19): Pi's decoder passes the failed turn's error to
`settle` instead of holding it on `Turn`; `harness::is_turn_in_progress`
(no caller on main either) is gone. `harness::pi` tests and clippy pass.

Concept review after slice 8 (2026-11-19, `scratch/concept-review.html`):
nothing changes. The question is read from the turn and `send_current` is
gone; the agent-move record in `agent_limit_recoveries` is the one small
mismatch left, larger than this Task.

Next: gate.

## Seen on the real CLI (OpenCode 2.2.1, 2026-11-18)

- `opencode run --standalone --format json --auto -m opencode/gemini-4-carbon`
  prints `step_start`, `tool_use`, `step_finish` and `text` lines, each with
  `sessionID` and a `part`; a refusal is one `error` line and exit 1. The trace
  is `harness/testdata/opencode_run.jsonl`.
- It reads the message from stdin when none is on argv.
- `--session` accepts any id starting with `ses` and creates the conversation;
  a bare UUID is refused ("Expected a string starting with \"ses\"").
- A run in another directory continues the same conversation; nothing needs
  moving first.
- The store names a turn by its `user` row. `run` gives no way to choose that
  id, so the driver reads the newest `user` row of the conversation when the
  first line arrives (`transcript::opencode::last_request`).
- Through the branch `lf` with an isolated `LF_HOME`:
  `lf -b -a opencode:opencode/gemini-4-carbon : "…"` answered, exited 0,
  recorded thread `ses_<capture>` and read usage from OpenCode's store
  (23,839 input tokens, $0.048). Asked which tool its instructions say launched
  it, the model answered "lf": the context plugin loads under `opencode run`.
- Not seen: the terminal client with `--prompt` on its own (Task
  conversations open it; nobody typed into one).

## Seen through `lf task run` on real CLIs (QA, 2026-11-18)

OpenCode 2.2.1 and Pi 1.3.0, both on `opencode/gemini-4-carbon`, branch `lf`,
private `LF_HOME` with its server, real `HOME`. `scratch/qa.html` has each run.

- `lf task run <ID> -a opencode:…` and `-a pi:…` each settled `implement`
  with a commit and stood at `human-review`; a mid-step `lf steer` changed the
  code on both. OpenCode usage is in `lf history usage --task`; Pi's is not
  (question 25, as predicted).
- `lf task interrupt` on OpenCode: exit 130, turn `interrupted`, no
  `opencode run` left; the next `lf task run` settled the step.
- `PATH` without `pi`: pinned, one attempt and the install hint; with
  `agents: [pi:…, opencode:…]`, the step ran on OpenCode and the move was
  recorded.
- Fixed in QA:
  - OpenCode roots a session in `$PWD`, not the process cwd. lf set only the
    cwd, so every worker's OpenCode session sat in the main checkout
    (`session_v2.directory`); its tools defaulted there. `configure_agent_env`
    now sets cwd and `PWD` for every headless harness; `NativeClient` sets
    `PWD` for terminals.
  - lf run from inside an OpenCode conversation merged that conversation's
    `OPENCODE_CONFIG_CONTENT` (its plugin, permissions) into the worker's, so
    the parent's context reached the worker first. OpenCode's markers and
    `OPENCODE_CONFIG_CONTENT` joined `CALLER_SESSION_ENV`; only `config.env`
    seeds OpenCode settings.
  - A steer the plugin claimed went into one request's system text; OpenCode
    keeps none, so later requests lost it and Gemini never acted on it. The
    plugin now resends every steer it has heard.
  - Every `lf task run -a` Graph log opened with "This command does not start
    an agent; --agent has no effect" (`graph run` was missing from
    `may_launch_agent`).
  - Doubled punctuation in the not-installed reason ("in Pi.. Inspect",
    "in Pi.; `lf task run`").

## What the rewrite changed for people

- A failed OpenCode start in a terminal prints `opencode exited with exit
  status: N`; the reason is in `opencode.stderr.log` beside the capture. The
  old "private server failed to start: <stderr>" line is gone.
- Reopening a conversation whose first launch failed continues the same
  capture, since lf named the conversation before it started.
- A run that dies mid-turn on its own records the turn `failed`, where the
  server's dropped stream recorded `interrupted`. One stopped by
  `lf task interrupt` or a signal to lf records `interrupted`.
- Headless OpenCode passes `--auto` always; lf no longer answers permission
  asks one at a time.

## Done when

`lf task run` on a Task pinned to OpenCode Gemini 4 Carbon, and on one pinned to
a Pi model, settles `implement` and moves to the next step; OpenCode chats run
`opencode --standalone` directly and headless batches/workers run `opencode run
--standalone --format json --auto` with obsolete `opencode serve` client/SSE
code deleted; pinning a harness that is not installed prints the install hint
after one attempt; the README table has a test or a named gap in every cell (slice 7 closes
the last five).
