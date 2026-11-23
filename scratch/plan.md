# LOO-1078 · Pause a Task without giving it up

> i think we need a 'paused' state / action for tasks -- stops the task worker - and ideally e.g. any rust compiling happening becasue of it - , maybe closes the tasks session for now as well, but oesnt unassign

## Done when

During a build, `lf task pause INF-123` stops the worker, its descendant build jobs (`cargo`/`rustc`), and the Task’s primary conversation process. The Task still belongs to the same person on the same Machine. Its dirty checkout, PR, Graph position, conversation identity, and history remain intact. Restarting the Machine server, running `wave-operate`, or receiving a Linear comment/steer leaves the Task quiet.

The demo runs two Tasks building side by side. Pause one: its build and nested build helpers exit, the other keeps compiling, and `lf task list --state paused` shows the first. On Mac and iPhone, a **Paused** view lists it outside Working without inflating active counts, opening it shows history and `"Paused at build. \`lf task unpause\` resumes it."` without launching a provider, and **Unpause** is offered as the primary action. Run `lf task unpause INF-123 --run` (or `lf task unpause INF-123` followed by `lf task run INF-123`): a fresh worker takes up the interrupted step with the existing files. At an interactive stop, `lf task unpause INF-123` returns the Task to Working and lets the server bring its primary conversation back (or `lf resume INF-123` attaches directly).

## Concept and data model

- **Pause applies to open, started Tasks.** A Task is started once it has a `task_checkouts` row or a `task_graphs` row (which includes `lf task create --run` during setup before checkout creation). `lf task pause` refuses `unstarted` Tasks (`"has not started; nothing to pause"`) and terminal (`done`/`abandoned`) Tasks. Keeping `unstarted` outside pause preserves Linear withdrawal (`TaskState::lifecycle`), `RoadmapTask.awaitsStart`, `RoadmapTask.isUnassigned`, and keeps Pause off Backlog items.
- **One hold per Task on its execution Machine.** Add a `task_pauses(task_id PRIMARY KEY, paused_at, paused_by, stopping_processes)` table on the execution Machine. Absence means runnable under ordinary rules. `stopping_processes` stores JSON `(pid, os_started_at)` entries captured before signaling so a crashed or retried pause can finish stopping reparented descendants and report what is still stopping; once all entries are confirmed dead, `stopping_processes` clears to `NULL`.
- **Lifecycle clears the hold cleanly.** `lf task unpause`, `lf task complete`, `lf task abandon`, `lf task reopen`, and `lf task unassign` clear `task_pauses`. Unassign refuses while `stopping_processes` still has live entries so a half-stopped build is never dropped to Backlog; once settled, unassign clears the hold so the Task returns to Backlog unpaused. `lf task move` carries the pause hold in `MovedTask` alongside `auto`, writes `task_pauses` on the destination before recovery observes the Task, and does not auto-continue a stopped Graph when paused.
- **`TaskState::Paused` and `TaskSection::Paused`.** Derive `TaskState::Paused` in `sqlite/durable.rs::task_state_sql` after terminal outcomes (`abandoned`, `done`) and before `moving`/`active`/`idle`, guarded to started Tasks (or ensured by pause only targeting started Tasks). Add `TaskSection::Paused` outside `Working` and `Backlog`, plus `SelectionState::Paused`. Add planning revision triggers on `task_pauses` and check the board query plan on the 10,000-Task fixture.
- **Explicit unpause gates all execution and wakes.** `lf task run`, `lf task auto on`, `lf resume --task`, `session.continue`, operator runs, steer wakes (including incoming Linear/Asana comments and child-Task notifications), and all three server recovery loops (`run_recovery`, `limit_recovery`, `due_task_conversations`) respect `task_pauses`. Reading status and opening a paused Task in Mac or iPhone have no launch side effect (Desktop also checks the hold when warming a selected or recently visited workspace; those paths can open a Session independently of `opensConversationAutomatically`). `lf task unpause <ID>` lifts the hold without replaying the stopped Graph; `lf task unpause <ID> --run` lifts the hold and immediately runs `lf task run <ID>` (matching `lf task create --run`). Pause leaves `task_auto` and existing external PR merge disposition alone.

## Shared worker teardown, settlement, and Session closure

- **Worker job ownership.** A shell can start a build in a new process group and exit before pause. Its orphan still belongs to the worker's OS session, but sampling the provider's descendants can no longer find it. Give each headless Task worker a fresh OS session before exec. The ordinary interrupt and the reaper can then stop the session's verified members, including reparented jobs. A daemon that creates its own OS session leaves this ownership scope. For a tool that runs Task work in a separate OS session, capture that tool's execution identity while it is known to the harness. Ancestry alone cannot distinguish it from a detached service.
- **Shared Task stop.** Pause, interrupt, and move must reach the worker's close path before its live provider identity is lost. Pause also needs the captured PID and birth pairs persisted before the interrupt is published, so settlement can finish after a crash. Interactive providers still share their terminal and need their own stop path.
- **Admission ordering and crash-safe retry.** Serialize `lf task pause` and Task run admission (`lf task run`, `lf task auto on`, recovery launches) under a per-Task admission lock (which covers the setup-before-checkout window too). Capture live Graph/step/AgentProcess roots and descendant group leaders while roots are alive, then commit the `task_pauses` row (with `stopping_processes`) and the interrupt event in one SQLite transaction. Release the admission lock and SQLite write lock before sending signals or waiting. If `lf task pause` crashes before finishing cleanup or a job survives, a repeated `lf task pause` (or server settlement pass) re-checks and stops the persisted `stopping_processes` entries even after their parent exited; `lf task unpause` refuses while any stopping entry or draining operation is live and names it. Shared compiler cache daemons, other Tasks’ builds, and unrelated shells in the same checkout have no Task-owned execution identity and are untouched.
- **In-flight operations and immediate run settlement.** Keep the existing interrupt rule for mutating operations already in flight (such as `lf land`): let the operation drain, start no later step, and distinguish draining operations from stopped workers/builds in the pause receipt. As soon as the worker/Graph stops during pause (not deferred to unpause), clear the carrier’s `run_pid`/`run_died_at` (marking it deliberately stopped) and end any active `agent_limit_recoveries` row so the paused Task never reads as a crashed run or a limit wait.
- **Primary Session closure via `Departure::Paused`.** Add `Departure::Paused { task: TaskId }` to `loopflow_storage::process::Departure` and pass it through `agent_process::stop_session_agent` when pausing. Live terminals display `"Task INF-123 was paused: lf task unpause INF-123"` via existing `departure_notice`. Add `Left::Paused` to `ops/recovery.rs::server_start` so paused conversations never restart and `session_ending` renders `"paused; \`lf task unpause INF-123\` resumes it"` instead of `"died (exit 143)"`. If the person already quit the conversation (`exit 0`/`130`) before pausing, pause does not overwrite that exit so unpausing keeps it quit; otherwise, unpausing lets the server’s existing recovery pass bring the primary conversation back in the background while retaining its thread ID, history, and any queued steers.

## Surfaces and single Rust truth

- **One sentence and one legality rule in Rust.** Update all four Rust projections from the same pause fact:
  - `TaskGraph.sentence`: `"Pausing at build (implement is stopping)."` / `"Pausing at deliver (pr-land is finishing)."` while draining; `"Paused at build. \`lf task unpause\` resumes it."` (or `"Paused at ship-review. \`lf task unpause\` resumes it."`) once settled.
  - `TaskRunControl.unavailable`: `"Task is paused; run \`lf task unpause <ID>\` to resume it"` (or naming what is still stopping while draining).
  - `TaskActionModel`: `recommended: NoAction`, `reason: "Task is paused"` (so `wave-operate`, `repo-operate`, and `lf wave show` leave paused Tasks alone).
  - `SessionOutlook` / `session_ending`: `Left::Paused` with the unpause command.
- **Discoverable Paused views on CLI, Mac, and iPhone.**
  - CLI: `lf task pause <ID>`, `lf task unpause <ID> [--run]`, `lf task list --state paused`, `PAUSED` section in `lf wave show`, served writes `task.pause` / `task.unpause`, remote routing to the execution Machine (unreachable owner → error; unknown Task → ordinary missing-Task error).
  - Swift (`TaskList`, sidebar, Wave board, `PendingChanges`): add `.paused` to `TaskList` (`state:paused`), add a discoverable Paused filter/section in Mac’s sidebar (`WorkNavigation.SidebarFilter` / `WorkNavigator`) and Wave board, and handle optimistic `pause`/`unpause` state transitions in `PendingChanges.swift`.
  - **`TaskActions` (`swift/Loopflow/Design/TaskActions.swift`):** add `pause: Pause?` (`.pause` for open, started, non-paused Tasks; `.unpause` for paused Tasks, disabled while draining with the refusal reason). While paused, omit `run` so **Unpause** is the clean primary action on Mac’s Workflow bar and iPhone’s Task actions menu; keep **Stop Auto-Ship** available if `auto == .on`, and keep **Auto-Ship** refused while paused.
- **Prior designs and compatibility.** PR #1544’s pause left processes running, PR #1652 renamed it defer, and LOO-618 replaced defer with unassign; the legacy `tasks.paused` column is marked leaving by LOO-938 in `migrations/plan_hub.sql` and stays untouched. Cover `task.pause`, `task.unpause`, `TaskState::Paused`, `TaskSection::Paused`, and `SelectionState::Paused` in shared Rust/Swift DTO fixtures, and update operator skills (`wave-operate`, `repo-operate`, `task-operate`) and Task docs.

**Choices made for review** (full comparison in `scratch/concept-review.html`):
1. **Pause started Tasks only (refuse `unstarted`).** Passed over pausing unstarted Backlog Tasks because it broke Linear withdrawal, `awaitsStart`, `isUnassigned`, and UI semantics.
2. **Explicit `lf task unpause` (with `--run` shortcut) required before `task run` / `resume`.** Passed over auto-unpausing on `task run` because parent-Task agents, cron `wave-operate`, and steer wakes would silently reverse a pause.
3. **Close primary Session via `Departure::Paused` on pause; let server restore it on unpause (unless already quit by person).** Passed over leaving the provider running (wastes RAM/CPU) or marking it quit (forces a manual `lf resume` after clicking Unpause).
4. **Dedicated `TaskSection::Paused` wired into `TaskList.paused` and Mac’s sidebar.** Passed over leaving paused Tasks in `Working` (inflates active count and operator passes) or omitting sidebar/phone lists (makes paused Tasks unreachable).
5. **Share descendant-aware worker teardown across `pause`, `interrupt`, and `move`.** Passed over building descendant teardown only for `pause` (leaves `interrupt` and `move` leaking nested builds).

## Build and gate

The first increment exercises the existing `lf task interrupt` entry point. Headless workers now own an OS session. Their close path applies TERM and KILL to its live members, including jobs already reparented when the interrupt arrived. In the macOS probe the provider was PID 40971, the job was PID and PGID 40987, its parent was PID 1, and its SID was 40971.

The CLI now records the hold and the worker-session members in the interrupt transaction. Run admission covers placement and carries the interrupt cursor into the Graph child. Worker and primary-provider launches take the same admission lock. A second lock serializes pause cleanup with unpause after admission reopens. A live command step keeps its carrier until it finishes. The CLI reads the new Paused state in lists and Wave sections.

1. **Worker ownership outside its OS session.** The Claude fixture runs its build in a separate process group inside the worker's session. This Machine's Codex tool shell ran in a separate OS session from its provider (see `scratch/questions.md`). Capture the kernel session ID where Codex starts the tool, before the command can fork. The 0.171.0 app-server notification and terminal list cannot supply this identity (probe below).

2. **Session closure, recovery, and ownership.** The primary closes with `Departure::Paused`. The launcher succeeds on a departure without recording a personal quit on the Session. Carrier, limit, and primary recovery skip the hold. A paused primary's outlook names the unpause command. Idle and stopped Session wakes consult the hold and leave incoming steers unread. Unassign, completion, abandonment, and reopening clear the hold in their lifecycle transactions. A move carries its pause time and author before publishing the destination claim and leaves a held Graph stopped. The source keeps its hold until arrival. Repeating a move after a lost reply keeps the choices made on the destination. Focused macOS tests cover a queued steer staying unread, unassign returning a settled Task to Backlog unpaused, a quit primary staying quit after unpause, and a retried pause stopping the saved build (see Checks below). CLI fixtures exercise pause after run admission and inside the pre-merge-commit hook of a draining `lf land --local`. The five-harness worker matrix goes to gate.
3. **Served writes and shared projections.** The execution Machine handles `task.pause` and `task.unpause`. A held Graph names the top-level step at its saved cursor, including when its carrier has exited. While a command drains it names the command. The board and status use the hold for the run refusal and the operator's NoAction recommendation. Swift's wire types include the two writes and the paused state, section and selection. The CLI's JSON pause receipt is the existing text receipt. JSON unpause returns null, or the ordinary run receipt with `--run`.

Mac’s sidebar requests only the selected section. Working and Paused have distinct readings and saved cache entries. The Wave board lists paused Tasks separately. iPhone reads `state:paused` under its Paused filter. Both clients offer plain Unpause, retain Stop Auto-Ship, and use the server’s `run_control.pausing` flag and refusal while cleanup runs. The Mac checks the hold when preparing recently visited workspaces. iPhone reads saved primary Session history while paused. Operator skills leave the hold in place. The CLI’s stop receipt identifies the Task and any finishing run. It does not enumerate stopped PIDs or retained files.

Remaining:

- **Codex tool sessions outside the worker's OS session.** The three probes below found no launch-side identity on Codex 0.171.0: the app-server reports `osPid: null`, a later census cannot tell the build from a double-forked service, and a zsh startup hook misses an explicit `/bin/sh`. Current choice (`scratch/questions.md`): such jobs stay outside signal authority and the PR names the gap. Open choice for review: accept that gap for this PR, or wrap Codex's tool command (a PreToolUse prefix, which can displace a configured rewrite) to register the session before the command runs.
- **Five-harness worker matrix and Linux session enumeration:** gate and CI.

## Pause during interrupt and move

- A worker attachment held across `lf task interrupt` kept the build alive until the pause caller entered cleanup. The Graph exited 130 at its original cursor and the separately detached service survived.
- A pre-push hook on the source reproduced a refused pause. The source had claimed the Task for mini, but mini had no checkout and refused the command. After the fix, mini received the saved cursor and hold, started no Graph, and the source build was gone. The source accepts pause while the move is leaving. Reading the hold and recording the sent payload share the cleanup lock. A send interrupted during cleanup can be retried after pause settles.
- The macOS CLI checks `pause_during_interrupt_holds_the_interrupted_step`, `pause_during_move_arrives_with_the_stopped_build_cursor`, and `served_pause_holds_the_remote_cursor_and_status_names_unpause` passed.
- `a_leaving_move_waits_for_pause_cleanup_before_sending` passed. A sender with unfinished pause cleanup kept the move and its hold. After settlement it recorded the send.
- The indexed move checks and four `documented_commands` tests passed, along with `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and the architecture map check.
- The move fixture pauses during the source push. A pause first issued after the payload is sent and before the receiver has a checkout can still need a retry on arrival.
- In a 10,000-Task SQLite probe (1,000 leaving), the claim lookup used `task_moves_task (task_id=?)` and took 2.12 µs per lookup including Python overhead.

## Codex launch identity and pause races

- A private Codex 0.171.0 app-server ran `exec_command` against the local Responses fixture on macOS. While shell PID 41822 was held open, `item/started` reported processId 90926 and `thread/backgroundTerminals/list` returned that handle with `osPid: null`. After shell exit the list was empty; build PID 41862 still occupied SID 41822. [Codex terminal projection](https://github.com/openai/codex/blob/rust-v0.171.0/codex-rs/app-server/src/request_processors/thread_processor.rs#L2655-L2684)
- In a second call, shell 49153 exited. Both survivors ran `/bin/cat` from the same tool environment, each leading a process group with parent PID 1. The build’s SID was 49153. The service’s SID was 49157, its already-exited launcher. The kernel still reported both session IDs. Only the fixture recorded which session Codex launched.
- A private zsh startup file registered the tool SID before the command ran. With the same configuration, explicitly selecting `/bin/sh` left build 39733 in the exited shell’s SID 39720 without a registration. A PreToolUse prefix can displace a configured command rewrite. [Codex hook ordering](https://github.com/openai/codex/blob/rust-v0.171.0/codex-rs/hooks/src/events/pre_tool_use.rs#L156-L172).
- `cargo nextest run -p loopflow --lib -E 'test(pause_after_run_admission) | test(pause_during_land)'` passed both tests on macOS. An admitted carrier meets pause before its Graph child executes. During a real local merge the pause receipt says the run is finishing, status names the command, and unpause refuses until the operation exits. Git merges the feature and the Graph skips its next step. `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed.

## Checks for Mac and iPhone pause controls

- `scripts/test_desktop.sh --skip-build --filter 'pausedTaskShowsInItsOwnListAndOpensOnUnpause|pausedTaskOffersUnpauseAfterCleanupAndKeepsAutoOffAvailable|boardPickerAndProjectionKeepServerOrderAndMembership|desktopInspectionRetainsUnavailableWorkAndExactPanes'` passed four tests after the Swift build. A paused row stayed out of Working, opened without entering the warm conversation set, and the Workflow bar’s Unpause button issued plain unpause.
- `uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests/TaskActionsTests/testPausedTaskOpensFromItsOwnFilterWithUnpause -only-testing:LoopflowiOSTests/TaskFeedTests/testUnpauseLeavesThePausedListAndARefusalBringsTheTaskBack` passed on an iOS 26.2 Simulator. The phone requested `kind:task state:paused`; a refused unpause returned the row to that list.
- `PendingChangesTests/pauseMovesBothListsAndARefusalRestoresTheRow` passed headless. The pending pause kept the owner and auto mark. A refusal restored the original row.
- Seven Rust checks passed across `a_pausing_task_names_the_work_to_wait_for`, `task_delivery_preserves_execution_and_due_unstarted_tasks`, `documented_commands::`, and `golden_prompts_match`. `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed.
- `uv run python scripts/check_swift_multiplatform_boundaries.py` and `uv run python scripts/check_architecture.py` passed.

## Checks for served pause

- `cargo nextest run -p loopflow --test integration -E 'test(served_pause_holds_the_remote_cursor_and_status_names_unpause) | test(dto_fixtures::selection_states_fixture_lists_every_state)'` passed on macOS. Mini held the cursor at implement, and the laptop's status showed “Paused at build.” Its board showed the refusal and NoAction. Plain unpause cleared the remote hold. An unreachable mini kept its hold. An unknown Task returned `not_found`.
- On a private Machine, CLI pause printed `"391d56 paused"` with `--json`. A socket `task.unpause` with `run: true` returned `{"issue":"391d56","state":"waiting","step":"demo"}` at the Graph's interactive step.

- Five focused Rust library checks passed on macOS: stopping and settled Graph sentences, the run refusal during cleanup, the screen and write fixture round trips, and the checked-in server schema. The draining sentence used the builtin command name `land --wait-and-fix`. A live landing operation remains a separate check.
- The three headless Swift checks passed: `coreLoopWritesShareTheRustWriteContract`, `screenReadsDecodeTheirCompletePayloads`, and `everyStateRustSendsDecodes`. `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `uv run python scripts/check_architecture.py` passed.
- With 10,000 Graphs, 1,000 held Tasks and 100 awaiting cleanup in SQLite, a cursor read looked up both rows by primary key (9.0 µs/read). Reading the pending workers for the 1,000 held IDs used the pause primary key (768 µs/read). The board skips that query when there are no held Tasks.

## Checks for the pause lifecycle

- The unpause check initially found `departure={"kind":"paused",…}` and `agent_exit_code=0` together on the stopped primary. Recovery treated that pair as a quit. After the exit fix, `cargo nextest run -p loopflow --lib -E 'test(task_graph_launch_tests::pause_keeps_a_queued_)'` passed in 21 s on macOS. The Machine server restarted the original Claude conversation and delivered the queued steer on unpause.
- `cargo nextest run -p loopflow --test integration -E 'test(retrying_a_move_after_unpause_keeps_the_destination_running) | test(a_move_that_did_not_arrive_shows_in_status_and_the_same_command_delivers_it)'` passed on two private Machines on macOS. One move lost delivery. The other lost its reply after arrival, and a retry kept the destination unpaused.
- On macOS, `cargo nextest run -p loopflow --lib -E 'test(pause_retry_stops_the_saved_build) | test(pause_leaves_a_previously_quit_primary)'` passed. Unassign refused with a live orphan recorded, repeated pause stopped it, and unassign returned the Task to Backlog. A primary that had exited with 0 retained that exit and stayed outside the restart queue.
- `cargo nextest run -p loopflow --lib -E 'test(pause_keeps_updates_off_an_idle_providers_socket)'` passed. Abandoning a child left its notification queued on the paused parent.
- `cargo nextest run -p loopflow-storage --lib -E 'test(completing_reopening_and_abandoning_a_task_clear_its_hold) | test(board_looks_up_each_hold_by_task_on_a_ten_thousand_task_machine) | test(reading_steers_for_starts_scans_nothing_growing)'` passed.
- An in-memory SQLite probe with 10,000 Task primaries and 1,000 holds found two indexed lookups for a paused primary (1.29 µs per call over 10,000 calls). Portable hold, cleanup and deletion used the pause primary key.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `uv run python scripts/check_architecture.py` passed on macOS.

## Checks for the pause CLI

- On the private macOS Machine, the target build observed its own PID in the committed pause record when TERM arrived. Its Graph exited 130. The neighbor remained alive and finished with exit 0. Restart and `unpause --run` reached the review step.
- A private CLI checkout refused pause on an unstarted Task. After placement, two pauses left one hold, `task list --state paused` found it, `task run` named the unpause command, and plain unpause lifted the hold.
- At 10,000 Tasks the admission and cleanup queries looked up the hold by its primary key. The worker query walked the partial live-agent index, with indexed Graph, capture and step lookups. Limit settlement used the Task's Graph and step indexes.
- `cargo test -p loopflow --lib task_graph_launch_tests::pause_stops_owned_build_and_keeps_claim -- --exact` passed on macOS (143 s). It covers two Tasks building side by side, plain `unpause` followed by `run`, `unpause --run`, and the primary conversation keeping its provider thread.
- `uv run python scripts/check_migrations.py` and `uv run python scripts/check_architecture.py` passed. Gate and CI cover the other worker harnesses and Linux session enumeration.

## Checks for the worker-stop increment

- `cargo test -p loopflow --lib task_graph_launch_tests::interrupt_stops_a_reparented_build_in_the_workers_session -- --exact` passed on macOS. The private Machine ran and interrupted a Claude stand-in. The build ignored TERM and died, the detached service survived, and the Graph recorded exit 130.
- `cargo test -p loopflow-execution --lib os_process::terminal_jobs_tests::shell_close_settles_background_and_stopped_jobs_without_touching_other_owners -- --exact` passed on macOS.
- `cargo test -p loopflow-execution --lib harness::agent_process::tests::detached_agent_is_reaped_but_duplicate_historical_identity_is_not_signal_authority -- --exact` passed on macOS.
- macOS session enumeration averaged 0.60 ms across 30 reads of 1,535 PIDs with one matching member. The probe timed the kernel calls and metadata read, excluding signal delivery.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `uv run python scripts/check_architecture.py` passed. Gate covers the other four worker harnesses, the lifeline, attachment, and hosted-terminal suites, and Linux session enumeration.

## User docs

Written ahead of implementation:
- `docs/work.md`
- `docs/surfaces.md`
- `docs/orchestration.md`
- `docs/sessions.md`
- `docs/troubleshooting.md`
- `docs/agent-api.md`
- `docs/lf-reference.md`
