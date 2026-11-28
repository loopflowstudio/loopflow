# LOO-1096 · Graph breakpoints

A worker finishes its step on its configured agent. At a breakpoint the Task’s primary conversation presents what it produced, links to the artifacts, gives its recommendation and asks numbered questions. Discussion can take several turns. Continuing accepts the default or carries the chosen direction to the next worker. Auto and batch take the default. Direct `lf <skill>` stays collaborative. The original choices and quotes are in [direction.md](direction.md).

## Pass 1 · What exists

The builtin research Graph runs `step: research` once per pass, then reaches `break`. The explicit skill reference avoids resolving the Graph of the same name. The runner saves the producing process and pending verdict in the cursor. Bypassing and continuing the marker both use the cursor’s existing transition. A Task run can park only once, so its carrier ID identifies the visit, including across repeats and Graph replacement. Parking releases the carrier and queues a Task steer in one transaction. Only the current primary can receive it while the visit remains current. The existing Machine watcher can start a quit primary in a held terminal. [Runner](../rust/loopflow-execution/src/graph/runner.rs) · [Parking](../rust/loopflow-storage/src/sqlite/task_work.rs) · [Delivery](../rust/loopflow-storage/src/sqlite/steers.rs)

Plain continue takes the stored verdict. The primary can send `--at-arrival`, `--decision` and `--reason` to change the decision at that visit. A fresh `--from` cursor would lose the selected path and repeat count. Mac and iOS Continue carry the arrival they displayed. Decision overrides and foreground ad hoc Graph review are implementation choices for review, recorded in [questions.md](questions.md).

**November 24 results, before the merge from main**

- `cargo test -p loopflow -p loopflow-execution --lib breakpoint_ -- --test-threads=1` passed four cases. In the CLI fixture, a Codex stand-in’s verdict with a path and numbered options reached the original Claude thread. The Machine server restarted **after delivery**. The CLI supplied “1b — compare keyboard use”, the next worker saw those words and pass two, the first arrival was refused, and plain continue reached end. Batch ran one worker and reached end with auto unset. A runner fixture retained the selected XOR path and pending verdict. The CLI fixture checks the input delivered to the primary. Opening the artifact and choosing an option through an actual conversation remain for the demo. [CLI fixture](../rust/loopflow/tests/work/graph_tests.rs)
- On a private Machine, an ad hoc foreground Graph printed “Continuing runs graph show research.” and stopped. Batch ran the command. Three served-write and eight server-schema checks passed. A 10,000-Task, 10,000-primary SQLite probe measured 34.1 μs per delivery eligibility read using indexed lookups.
- Four `documented_commands::` checks and `website/tests/test_readme_index_sync.py` passed. The changed Swift files parsed and a standalone production decoder/action returned Continue with the fixture arrival. Desktop file opening and state inspection timed out. Local all-target Clippy stopped after 24 minutes under load without a result. Gate owns Clippy and the Swift package check.

## Pass 2 · Existing and new Tasks now reach their first review

The feature, code, redesign and local kickoff-demo Graphs now run their workers before the break. The old YAML spelling compiles the same way after capturing the occurrence’s settings. A fresh feature run starts at explore. Batch and auto bypass the markers. A blocked decision at a marker leaves its reason and arrival available for a reply.

Released Task captures keep their original definition while a carrier owns them. Continuation resolves a named old wait to its pending worker, maps entries, end positions, return edges, XOR children and repeat counts together, and claims the original row. SQLite fixes the Graph at take-up. A successful claim replaces that row with the converted definition and checkpoint in one transaction. A Machine move can carry either representation, and numeric process history continues to use its own captured Graph. LOO-1100 owns removal of the released readers. [Conversion](../rust/loopflow-execution/src/task_graph/legacy.rs) · [Task start](../rust/loopflow-execution/src/store/task_work.rs) · [CLI fixtures](../rust/loopflow/tests/work/graph_tests.rs)

**November 25 checks**

- The compiled CLI ran both cases on a private Machine with a scripted Codex provider. A new `lf task run <ID> feature` ran explore once and parked at `after-explore` (index 1). A released capture with two identically named workers ran its pending explore (node 1), parked at index 2, and reached `end` on plain continue with exactly one worker in its history.
- Eight focused execution tests passed, covering the nested checkpoint, moved capture, stale claim and Task previews. Four compiler and catalog tests passed for occurrence settings, composition and review order. `lf graph show` rendered a released occurrence ID and a loop with their expected breakpoints.
- Both `graph_tests::breakpoint_runs_` fixtures passed. The original primary received the recorded result in each case, and the released Graph history kept its numeric step keys. The foreground terminal accepted and declined a review, and a blocked auto run continued after a reply without rebuilding. `cargo fmt --check` passed.
- Both claim statements used the Task primary-key index with 10,000 Task Graph rows. Updating and replacing one small capture took a median 21.7 μs over 500 in-memory rollback samples. This excludes disk commit time.
- All three workflow-card comparisons (feature, code and research) passed in Chromium against the homepage served by the in-process website app. The pytest server fixture timed out starting its reload subprocess before it served a page.

## Pass 3 · Skills, delivery, readback and layout handoff into planning

Updated the builtin skills (`explore`, `kickoff`, `design`, `ship-review`, `verify`, `demo`, `task-session`, `task-operate`, `advance`, `launch-plan`, `wave-operate`, `repo-operate`), repository overlays (`.lf/skills/demo.md`, `.lf/skills/verify.md`), common headless instructions (`surfaces/headless.md`), and prompt goldens to the two-mode breakpoint model. Direct invocation collaborates; headless work writes its artifacts (`scratch/explore.html`, `scratch/plan.md`, `scratch/ship-review.html` + `scratch/tour.yaml`, `scratch/verify.md`, `scratch/demo.md` + isolated `.dev` bundle ready to open), poses numbered choices, and states the recommended next action without merging, completing, or filing optional follow-up Tasks ahead of review.

Extended breakpoint review readback ([graph.rs](../rust/loopflow/src/lf/commands/graph.rs), [graph_inventory.rs](../rust/loopflow-storage/src/sqlite/graph_inventory.rs)) to report recorded command step results (label, process ID, outcome, exit code, error, and Graph log path), resolve `scratch/...` links to absolute paths on the execution Machine, report missing referenced artifacts alongside what is available under `scratch/`, fall back to retained turn answers when a provider transcript is absent, and name the forward destination (or `end`) for leading, adjacent, and terminal breakpoints without a producer. Fixed concurrent `auto on` vs. breakpoint reply claiming in [ops/task.rs](../rust/loopflow-execution/src/ops/task.rs) and the stale `Mutation::TaskRun` initializer in [machine_commands.rs](../rust/loopflow/tests/machine_commands.rs).

**November 25 checks**

- `breakpoint_feature_kickoff_` passed all three modes (`Reply`, `Continue`, `Auto`) through the real `feature` Graph into `kickoff`: a reply delivered both `Recommendation:` and the chosen layout direction; plain continue and `auto on` delivered the recommendation without the reply after a single `explore`.
- `breakpoint_direct_graph_in_task_checkout_leaves_task_alone` and `breakpoint_runs_the_released_pending_worker_once` passed against `target/debug/lf`.
- `breakpoint_review_reads_commands_resolves_artifacts_and_handles_missing_sources` passed: verified recorded command step detail + log path, resolved execution-Machine paths, missing artifact fallback listing available `scratch/` files, missing provider transcript fallback to retained answer and graceful unavailable message, and leading/adjacent/terminal producerless breakpoints.
- `breakpoint_arrival_scoping_lifecycle_and_query_plans` passed (0.50s): verified arrival before primary creation, sibling/worker isolation, simultaneous reopen taking once, delivered arrival staying delivered after quit, later visit waking once, and zero growing-table scans in `query_plans::growing_scans` for both candidate (`read_steer_wakes`) and eligibility queries. A 10,000-Task / 10,000-primary / 10,000-steer SQLite probe measured 3.4 μs median for `read_steer_wakes` candidate discovery using indexed primary-key lookups.
- `breakpoint_delivery_covers_five_harnesses_busy_restart_and_failed_start` and `a_parents_conversation_hears_when_a_child_parks_at_a_breakpoint` passed: covered open-idle wake for Claude, Codex, OpenCode, and Pi, Antigravity next-turn `claim` and held-start (immediate idle wake remains deferred to LOO-1098), busy delivery after turn end, server restart before delivery, failed start retaining unread arrival, and parent Task notices.
- `breakpoint_edges_cover_launch_failure_terminal_auto_race_and_named_restart` passed (1.86s): covered launch failure after accepting a reply, terminal settlement + completion request, auto racing a late reply at a repeated marker without publishing losing direction, and deliberate restart targeting a named occurrence with `--at-arrival`.
- `builtins::tests` (12), `prompt::tests::format_prompt_` (9), `documented_commands::` (4), `golden_prompt` (updated 6 goldens), and `cargo fmt --check` all passed.

## Pass 4 · Unified breakpoint projection, Now/sidebar priority, docs and QA

Replaced the remaining interactive-skill display paths with one breakpoint projection from Rust through Task status, catalog, Graph show/list, explain, preview, terminal title, Mac and iOS:

- **One walk and wire shape:** Removed `InteractiveStep`, `interactive_steps`, `WorkflowStep.interactive`, `GraphNode.interactive`, and the dead `TaskRunAction::Wait` variant. `WorkflowOutline` and `TaskGraph` now compute `breakpoints: Vec<String>` in a single walk (including composed Graphs, XOR paths, and unconverted legacy `skill.interactive` captures until LOO-1100) and derive `WorkflowOutline.sentence` directly from it. Added shared `breakpoint_sentence(step, &arrival)`, updated `TaskGraph::waiting_step()` and `waiting_to_land()` for `TaskGraphPosition::Breakpoint`, updated `terminal_title::standing()`, `lf graph show` (including return arrows `(returns to <target>)`), `lf graph <name> --context` (`[break]`), and `GraphDiagramView` (`.pendingBreakpoint`). [TaskGraph](../rust/loopflow-execution/src/task_graph.rs) · [Compiled](../rust/loopflow-execution/src/graph/compiled.rs) · [Swift Models](../swift/Loopflow/Models/TaskWork.swift)
- **Now, Active priority and shelving precedence:** Added `SelectionState::Breakpoint` (`"breakpoint"`) to Rust and Swift, derived in `sqlite::selections` via an indexed primary-key `EXISTS` check on `task_graphs` and gated on `TaskState::Active` so future shelving (LOO-1078) takes precedence while retaining the parked cursor. Added `SelectionState::Breakpoint` to `SelectionResolution.attention`. `waves.rs` reads `task_graphs.breakpoint` in the existing `task_details` query (zero extra SQL queries) and projects `condition` (`Waiting` or `Blocked` for blocked verdicts, with `breakpoint_sentence`, even when the checkout is clear), `next_move` (`NextMoveOwner::User`), and `actions` (`TaskAction::Run` unless refused). In Swift, `RoadmapTask.need` keeps breakpoints in Now, `NowProjection` places breakpoints ahead of non-breakpoint peers (preserving oldest-first peer order), `WorkNavigation.StatusGroup` puts `.breakpoint` first when **Sort by status** is on while restoring saved drag order when toggled off, `RoadmapTaskAction.run` labels breakpoints `"Continue"`, and `BacklogView` / `TaskWorkView` read `workflow.breakpoints`. [Waves](../rust/loopflow/src/lf/commands/waves.rs) · [Selections](../rust/loopflow-storage/src/sqlite/selections.rs) · [Now](../swift/Loopflow/Models/NowProjection.swift) · [Sidebar](../swift/LoopflowMac/WorkProjection.swift)
- **Fixtures, schema, docs and QA:** Migrated 18 DTO fixtures under `tests/fixtures/dto/`, regenerated `tests/fixtures/dto/server_schema.json`, and updated help copy, [reference-models.md](../docs/dev/reference-models.md), [AGENTS.md](../AGENTS.md), and `.lf/qa/{auto,iphone-simulator,opencode-pi-workers,task-children,task-conversations}.md`. Manual sidebar ordering when **Sort by status** is off remains an open choice in [questions.md](questions.md).

**November 25 checks**

- Focused Rust unit tests passed: `loopflow-storage` (`selections::tests::` 2, `work::selection_tests::breakpoint_selection_state_parses_and_enters_attention` 1, `query_plans::` 17), `loopflow-execution` (`task_graph::` 7, `graph::compiled::` 10, `graph::transitions::` 10, `ops::task::explain::tests::` 6, `terminal_title::tests::` 7, `store::task_work::tests::` 2), and `loopflow` (`lf::commands::waves::tests::task_condition_` 2, `lf::commands::graph::tests::render_pipeline_lines` 1, `lf::commands::graph::tests::pipeline_lines_mark_breakpoints` 1, `server::schema::tests::` 8).
- Rust integration checks passed: `dto_fixtures::` (50/50), `documented_commands::` (4/4), and `cargo fmt --check`.
- Focused Swift tests via `scripts/test_desktop.sh` passed: 30/30 tests across 8 suites (`DTOFixtureTests`, `NowProjectionTests`, `WorkflowLineTests`, `BacklogViewTests`, `RoadmapViewTests`, `TaskBriefTests`, `TaskGraphTests`, `WorkNavigationTests`) plus `WorkSelectionTests/everyStateRustSendsDecodes` (1/1).
- Compiled CLI checks confirmed `lf graph show feature`, `lf graph show research` (with `(returns to research)`), `lf graph list --json` (`workflow.breakpoints` and `workflow.sentence`), and `lf graph research --context` (`1: after-research [break] (returns to 0)`).

Realign after Pass 4: the removed types are gone from Rust and Swift. `ConcreteSkill.interactive` and `interactive_name` remain only for unconverted released captures (compilation always writes `false`), as does the `occurrence.interactive` branch in [instructions.rs](../rust/loopflow-execution/src/graph/instructions.rs). LOO-1100 removes them with the released readers. Twenty stale "interactive step" doc comments in storage, execution, auto, run recovery and the server/harness READMEs now say breakpoint.

## QA · November 25, real providers

The feature walkthrough ran on real Claude: explore on `claude:sonnet`, the primary on the default model. The primary was crashed during explore and restarted, the Task parked at `after-explore`, and the handoff was presented with the artifact and numbered choices. The reply “1b 2b 3b, I want the switch with a warm kitchen palette” reached kickoff through `--at-arrival`/`--reason`. Two fixes came out of it: the primary had blocked in `lf task wait --until submitted` for the whole review, which Pass 5 replaced with `--until breakpoint` (task-session now uses it; `submitted` again waits only for an open PR or a finished Task), and status says a parked run “stopped at breakpoint” instead of “finished”. Details and what remains wrong are in [qa.html](qa.html).

## Pass 5 · Unified breakpoint ownership, navigation, reply scope, and waits

Closed all four concept splits identified at `4b295b5e3a` ([concept-review.html](concept-review.html)):

1. **One breakpoint owner across CLI `lf task status`, Desktop/Roadmap, selections, and blocked runs:**
   - Added `BreakpointActionEvidence { blocked, sentence }` to `TaskActionEvidence` in [task_actions.rs](../rust/loopflow-execution/src/ops/task_actions.rs), taught `Finished::of` that a parked breakpoint is never `Finished::Merged`, and taught `derive_task_actions` to emit `TaskAction::Run` (unblocked) or `TaskAction::NoAction` (blocked) with the breakpoint sentence while still respecting `launch_refusal`, `run_refusal`, and abandoned predecessors.
   - Passed breakpoint evidence from [snapshot.rs](../rust/loopflow-execution/src/ops/task/snapshot.rs) into `derive_task_actions` and removed the duplicate breakpoint action bypass in [waves.rs](../rust/loopflow/src/lf/commands/waves.rs).
   - Recognized parked `task_graphs.breakpoint` rows in both `snapshot.rs` and `waves.rs` when `execution.state == Idle` or when `execution.state == Blocked` and the parked arrival carries a `blocked` verdict, keeping `SelectionState::Breakpoint`, `NextMoveOwner::User`, `TaskConditionState::Blocked`, and `TaskAction::NoAction` in agreement with `sqlite/selections.rs`.
   - Carried `blocked: bool` on `TaskGraphPosition::Breakpoint` across Rust ([task_graph.rs](../rust/loopflow-execution/src/task_graph.rs), [store/task_work.rs](../rust/loopflow-execution/src/store/task_work.rs)), Swift ([TaskWork.swift](../swift/Loopflow/Models/TaskWork.swift), [TaskActions.swift](../swift/Loopflow/Design/TaskActions.swift)), [task_graph_breakpoint.json](../tests/fixtures/dto/task_graph_breakpoint.json), and [server_schema.json](../tests/fixtures/dto/server_schema.json), omitting the plain **Continue** button on blocked breakpoints.

2. **Navigation (`Start from…`, `--from <loop-breakpoint>`, and copy):**
   - Added `TaskGraph.startFromSteps` and updated Desktop’s **Start from…** menu in [TaskWorkView.swift](../swift/LoopflowMac/Views/TaskWorkView.swift) to list `start` plus top-level runnable work stages (excluding standalone `breakpoints`).
   - Updated [ops/task.rs](../rust/loopflow-execution/src/ops/task.rs) and [lf/commands/graph.rs](../rust/loopflow/src/lf/commands/graph.rs) to check `return_target(body, leaf.index).is_some()` for loop-decision eligibility and handoff advertising, constructing a `GraphVerdict` when none was recorded (e.g. after CLI `--from after-ship-review`) and requiring `--reason <direction>` when iterating without a prior verdict.
   - Replaced `"Auto stands in next at {next}."` with `"Auto continues through {next}."` in [task_graph.rs](../rust/loopflow-execution/src/task_graph.rs) and its tests.

3. **One clean scope for breakpoint `--reason` replies:**
   - Added `GraphProgress.handoff` ([transitions.rs](../rust/loopflow-execution/src/graph/transitions.rs), [runner.rs](../rust/loopflow-execution/src/graph/runner.rs), [ops/task.rs](../rust/loopflow-execution/src/ops/task.rs)) for single-step non-loop / `advance --reason` replies (cleared as soon as the immediate next step settles, preserved across retries/restarts, never copied into XOR children or leaked downstream), keeping `GraphProgress.direction` strictly for loop iteration.
   - Recorded breakpoint replies in `task_comments` with `<!-- loopflow-breakpoint:{arrival} -->` (`append_breakpoint_reply_comment`) so the person’s exact words stay once in history without inserting a duplicate steer into `steers` (while non-breakpoint and `auto on --reason` runs keep steering as before).

4. **Wait cleanly for a breakpoint even when a PR is already open:**
   - Added `TaskWaitUntil::Breakpoint` (`lf task wait --until breakpoint`, wire `"breakpoint"`, returning on parked breakpoint, Graph at `end`, or terminal even when a PR is open) and kept `--until submitted` (`TaskWaitUntil::Open`) for PR submission / terminal across [ops/task.rs](../rust/loopflow-execution/src/ops/task.rs), [lf/mod.rs](../rust/loopflow/src/lf/mod.rs), [bin/lf.rs](../rust/loopflow/src/bin/lf.rs), [commands/read.rs](../rust/loopflow/src/lf/commands/read.rs), [task_session.md](../rust/loopflow-definitions/src/builtins/task/skill/task_session.md), [server/README.md](../rust/loopflow/src/server/README.md), and [RegistryQuery.swift](../swift/Loopflow/Services/RegistryQuery.swift).

**November 25 Pass 5 checks**

- Focused `loopflow-execution` tests passed (42/42): `ops::task_actions::`, `task_graph::`, `graph::transitions::`, `graph::runner::tests::breakpoint_handoff_`, `ops::task::tests::task_wait_condition`, `ops::task::tests::breakpoint_`, `ops::task::tests::task_snapshot_owns_breakpoint_actions`, and `ops::task::explain::tests::`.
- Focused `loopflow` unit and integration tests passed: `lf::commands::waves::tests::{task_condition_,blocked_auto_breakpoint_}`, `server::schema::tests::` (8/8), `lf::tests::` (38/38), `dto_fixtures::` (50/50), `documented_commands::` (4/4), `golden_prompt` (1/1), `graph_tests::breakpoint_feature_kickoff_receives_the_chosen_layout` (verifying single delivery without double-steering), and `task_graph_launch_tests::auto_on_at_the_last_breakpoint_completes_without_another_graph_run`.
- Focused Swift tests passed: 107/107 tests across 6 suites (`TaskActionsTests`, `WorkflowLineTests`, `DTOFixtureTests`, `RegistryQueryTests`, `DashboardWorkspaceTests`, `DesktopHeadlessTests/taskWorkflow`), plus `ServerProtocolTests` against the regenerated `server_schema.json`.
- `cargo fmt --check` passed.

## QA · November 25, after Pass 5

Real Claude for primaries and `claude:haiku` workers on a private Machine. A primary that quit with `/exit` was started again at the breakpoint and presented the worker's artifact and question. A reply choosing the option that wasn't recommended reached the next worker word for word. `--until breakpoint` returned at the park; `--until submitted` did not. Iterating at a revisited loop breakpoint required `--reason`. Three fixes: a breakpoint reply was sent back to the primary as a "new comment" update with raw markers (`task_undirected_comments` now skips it); continuing a breakpoint printed "from start" (it now names the step); and pass 1 of a loop was told to compare with a preceding pass, which the worker made up (the sentence now starts at pass 2). Details are in [qa.html](qa.html).

## Pass 6 · Ship-review changes

Synced with main at `6deb3b0450` in `94901da89e`. Follow-up fixes are in
`dca368a68e`. Feature and code use the
review loop before delivery. The post-merge verify stop and the outer loop
are gone. Explore and ship-review compile a review page for the Task
conversation. Main's pause holds a breakpoint at the same visit, with
Unpause returning to review.

Roadmap Continue carries `breakpoint_arrival` through to `--at-arrival`.
Blocked reviews show the worker's reason in the workflow bar. The person
icon stands on its own for a standalone breakpoint. A composition keeps
its stage name. Plain Continue uses the recorded recommendation. Status
sorting puts breakpoints first within Active, and manual sorting keeps
the saved order. The parser points retired `human:` items to `- break`.

### Checks

- `cargo build -p loopflow --bin lf` passed. The CLI showed the builtin
  feature, code, quick and research Graphs. A disposable command-only Graph
  presented its review in foreground and crossed it in batch. The retired
  `human:` item reported the new `- break` guidance.
- The 161 focused Desktop checks passed, including the Roadmap and workflow bar
  Continue commands, the blocked review, pause and Now ordering, both DTO
  decoders, and the regenerated server schema.
- 84 focused Rust checks passed across three runs. The end-to-end fixtures
  exercised the feature reply into planning, a draining command parking at
  a breakpoint during pause, and unpause preserving the recorded arrival.
  Schema, DTO, prompt and documented-command checks passed.
- The Task Graph and Roadmap reads use the Task primary-key index on 10,000 rows.
  In-memory medians were 1.7 µs for one Task and 986 µs for 250 Roadmap rows.
- `uv run --project website --extra test pytest -q website/tests/test_readme_index_sync.py` passed.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  and Swift parsing passed.

The next Graph step owns publishing and delivery. LOO-1098 and LOO-1100
remain separate. The November 27 review dropped the five pre-existing QA
edges. [Exact reply](direction.md#exact-words--ship-review-jack-heart-2026-11-27)

## Pass 7 · Align workflow track markers, paused/blocked chips, and paused breakpoint wakes

Post-build concept review at `59f26fa51d` ([concept-review.html](concept-review.html)) found two model splits introduced during Pass 6’s merge with `lf task pause` and `TaskWorkflowControls`:

### Experience and model

1. **Workflow track items carry their own kind, and blocker text stays off the chip label:**
   - A top-level track item is one of three kinds (`WorkflowStepKind`: `stage`, `stage_with_breakpoint`, `breakpoint`), with no impossible state and no Swift guessing via `breakpoints.contains(step.name)`:
     - `stage`: ordinary runnable stage (`explore`, `plan`, `build`, `ship-review`, `deliver`). Renders its name, appears in **Start from**, and is clickable on the track.
     - `stage_with_breakpoint`: runnable composed stage (or unconverted legacy interactive skill until LOO-1100) containing a breakpoint. Renders `👤 <name>`, appears in **Start from**, and is clickable on the track — even if a breakpoint inside it shares its name.
     - `breakpoint`: standalone top-level `- break` marker (`after-explore`, `after-ship-review`). Renders only `👤` (no `"after-explore"` text), does **not** appear in **Start from**, and is a non-clickable marker on the track (tooltip `"Breakpoint: <name>"`), never a `Button` launching `--from <breakpoint>`.
   - `TaskGraphPosition::Breakpoint` carries `blocker: Option<String>` (`#[serde(deserialize_with = "Option::deserialize")]`) instead of `blocked: bool`, and `WorkflowControl.label` stays a short chip label (`Blocked: <chip>`, matching `Paused: <chip>` and `Waiting: <chip>`) instead of smuggling the worker’s blocker sentence.
   - In Swift `TaskActions`, `breakpointArrival` and `breakpointBlocker` are populated only when `!held` (`row.lifecycle != .paused`). When a Task is paused or pausing at a breakpoint, the chip always displays `actions.control?.label` (`Paused: after-explore` / `Pausing: after-explore`) instead of bare `👤 ⌄`. When unpaused and blocked, the primary button is hidden, the chip shows `👤 ⌄` (standalone) or `Blocked: <stage> ⌄` (composed), and the blocker text renders **once** beside the track.

2. **Pause holds breakpoint handoffs; Unpause delivers an unread parked review once:**
   - `steers.rs:scopes()` gates `breakpoint:%` steers on `AND NOT EXISTS(SELECT 1 FROM task_pauses p WHERE p.task_id=tp.task_id)` (indexed PK lookup on `task_pauses`), matching `task_state_sql`, `selections.rs`, `waves.rs`, and `snapshot.rs`. A paused Task never wakes an idle primary or starts a quit primary in a held terminal for a breakpoint while shelved.
   - In `sqlite/task_pause.rs:release_task_pause`, when `settle_step.is_none()` and `task_graphs` is parked at a breakpoint (`run_start IS NULL AND breakpoint IS NOT NULL`), look up the `breakpoint:<task>` steer for that arrival ID. If it has no delivered receipt (`NOT EXISTS(SELECT 1 FROM steer_delivered WHERE steer_id=?1 AND delivered_at IS NOT NULL)`), delete any unstamped `steer_delivered` rows and the old `steers` row and re-save it via `steers::save_in` inside the same transaction (after deleting `task_pauses`). This gives the unread review a fresh `seq`, bumps revisions, and touches the steer marker so the watcher wakes or held-starts the primary once on unpause — while leaving already-delivered reviews quiet.
   - An idle wake also publishes generic Task progress before claiming steers. At a held breakpoint, record the Graph's progress in the observation and let the review carry the arrival and its run outcome to the primary on unpause. Keep comments, PR changes, and notices to conductors flowing. Exercise the idle wake as well as the stopped-conversation queue.

### Implementation

- **Slice 1 (Rust DTO + Swift workflow controls — proves the UI/wire model immediately):**
  - `rust/loopflow-execution/src/task_graph.rs`: replace `WorkflowStep.breakpoint: bool` with `WorkflowStep.kind: WorkflowStepKind` (`stage` | `stage_with_breakpoint` | `breakpoint`); replace `TaskGraphPosition::Breakpoint.blocked: bool` with `blocker: Option<String>`; set blocked `WorkflowControl` label to `format!("Blocked: {chip}")`; update `ops/task/snapshot.rs` and `store/task_work.rs` (`blocker.is_some()`).
  - `tests/fixtures/dto/` & `tests/fixtures/dto/server_schema.json`: update `WorkflowStep` (`kind`) and `TaskGraphPosition::Breakpoint` (`blocker`) across affected fixtures and regenerate the server schema.
  - `swift/Loopflow/Models/TaskWork.swift`: mirror `WorkflowStepKind` (`TaskGraph.Step.Kind`) and `Position.breakpoint(step:arrival:blocker:)`.
  - `swift/Loopflow/Design/WorkflowLine.swift`: drive `WorkflowStepLabel` directly from `step.kind` (drop `standalone: Bool` and `WorkflowLine.breakpoints`).
  - `swift/Loopflow/Design/TaskActions.swift` & `swift/Loopflow/Design/TaskWorkflowControls.swift`: gate `breakpointArrival`/`breakpointBlocker` on `!held`, build `steps` from `track.filter { $0.kind != .breakpoint }`, render standalone breakpoints on the track as non-clickable markers (not `stepButton`), and show `👤 ⌄` in the chip only when `!held` and `step.kind == .breakpoint`.
  - Tests: update `task_graph::`, `dto_fixtures::`, `server::schema::tests::`, `WorkflowLineTests`, `TaskActionsTests` (add explicit cases for paused-at-breakpoint chip label, non-clickable standalone breakpoint marker, composed stage sharing a breakpoint’s name, and blocked breakpoint inside a composed stage rendering the blocker once), `DesktopHeadlessTests`, and `ServerProtocolTests`.
- **Slice 2 (Storage pause/unpause breakpoint handoff lifecycle):**
  - `rust/loopflow-storage/src/sqlite/steers.rs`: exclude paused Tasks (`NOT EXISTS(SELECT 1 FROM task_pauses ...)`) in `scopes()` for `breakpoint:%` steers; verify `query_plans::growing_scans` remains zero scans.
  - `rust/loopflow-storage/src/sqlite/task_pause.rs`: re-publish an unread parked breakpoint steer inside `release_task_pause` when `settle_step.is_none()`.
  - Tests: add storage/execution tests covering (a) draining command or unread breakpoint arriving while paused does not appear in `unread` / `read_steer_wakes`, (b) `release_task_pause` re-publishes an unread breakpoint steer once and wakes the primary, and (c) an already-delivered breakpoint steer is not re-delivered after pause + unpause.

### November 27 checks

- Both slices are implemented. The 104 focused Rust checks passed, including the CLI draining-command pause and unpause, the idle primary's watcher, stopped-primary wake eligibility, and the case where the review was already read before pausing. The schema, DTO, command docs, and formatter checks passed.
- The focused headless Swift run passed across 10 suites and reported 143 tests, with the two optional display checks skipped. View tests cover the held chip, a standalone marker with no start action, a stage sharing a breakpoint name, and a blocker shown beside the track.
- The SQL plan test found no growing-table scans. In an in-memory probe with 10,000 Tasks and primary Sessions, 40,000 steers, and 2,500 holds, median reads were 19.4 µs for paused eligibility, 10.1 µs for unpaused eligibility, and 2.2 µs for locating the review on unpause.

## Remaining

- Deferred to delivery: full CI suite run on the published head of PR #2042. LOO-1098 (Antigravity immediate idle wake) and LOO-1100 (retiring legacy `interactive:` readers) remain separate.
