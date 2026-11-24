# LOO-1078 · Pause a Task without giving it up

> i think we need a 'paused' state / action for tasks -- stops the task worker - and ideally e.g. any rust compiling happening becasue of it - , maybe closes the tasks session for now as well, but oesnt unassign

## Revision after demo & ship-review

Stacked on `LOO-1063` (Working sidebar order + status sort). Keep the `stopping_sessions` rename and the documented Codex 0.171.0 / post-send move limits; revise the implementation as follows:

1. **Implemented — keep the Task Session open on pause.**
   - `lf task pause` stops only the Task's worker / Graph run, **any subprocesses in the worker's OS session** (describe generically in code/docs, not cargo-specific), and scheduled worker/limit retries.
   - Do **not** close the primary interactive Task Session on pause. Remove `Departure::Paused`, `Left::Paused`, primary-session teardown during pause, and Desktop's paused-conversation skip when opening/warming a Task.
   - While paused, the open Task Session keeps working normally (steers/messages reach the open conversation), while Graph runs (`lf task run`, `lf task auto on`, carrier recovery, limit recovery, `wave-operate`) stay held until explicit `lf task unpause`.
   - Four focused Rust tests passed on macOS. In two simultaneous runs, the paused worker's subprocess exited, the neighboring run and the primary conversation survived, and unpause reached review. A crashed conversation restarted during the hold. An idle one received a steer and a child-Task update. Pausing during run admission stopped the first step.
   - The server schema and four documented-command checks passed. The headless Mac test opened the paused Task and kept its conversation warm. The iOS 26.2 Simulator opened the Session terminal from the paused row.
   - Two test fixtures still used the pre-stack APIs for Working-list drops and the iPhone feed. The updated refusal tests passed on Mac and iPhone.
   - `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and the architecture and Swift module checks passed. Gate and CI cover the wider suites.
   - A Task-bound standalone turn also polls interrupt events. Pause now records a Graph stop in the same transaction as the hold. The Graph checks only events after its launch cursor. In SQLite with 10,000 Tasks and one million events, the poll used `idx_task_events_task (task_id=? AND id>?)` and took 3.06 µs with no new event.

2. **Implemented — keep paused Tasks inside `TaskSection::Working`, with 3 sections in the left Working list.**
   - Owned paused Tasks remain in `TaskSection::Working` (not a separate `TaskSection::Paused` or separate Paused sidebar filter / Wave board section). Remove `TaskSection::Paused`, `TaskList.paused`, the Mac sidebar Paused filter, the Wave board / `lf wave show` `PAUSED` section, and the iPhone Paused filter. Keep `lf task list --state paused` filtering by `TaskState::Paused`.
   - On top of `LOO-1063`'s Working sidebar order and status-sort toggle, group owned Working Tasks in the left sidebar into **3 sections**:
     - **Active** — owned working Tasks not in Shipping or Paused
     - **Shipping** — `auto == on` OR no interactive step left ahead before landing (compute one shared predicate in Rust and carry it on the wire; Mac, iOS, and CLI read it)
     - **Paused** — owned paused Tasks (`TaskState::Paused`)
3. **Implemented — make `verify` an automatic step in `code` and rewrite it to stay fast.**
   - Change `verify` in the builtin `code` graph from `interactive: true` to an ordinary automatic step.
   - The candidate CLI rejected the old outer loop: `loop at verify returns across interactive step "ship-review"; a person decides there`. Keep the build/review loop and end both Graphs with deliver, then automatic verify. A failed merged claim gets a linked fix-forward Task.
   - Rewrite the builtin `verify` skill (`rust/loopflow-definitions/src/builtins/task/skill/verify.md`) so it runs whatever verification it can **quickly** after merge and never blocks indefinitely (e.g. waiting for a production release/deploy).
   - With `verify` automatic, the existing "no interactive step left ahead" rule cleanly aligns with **Shipping** once a Task passes `ship-review`.
4. **Redesign the top-right Task bar (`TaskWorkflowBar` / `TaskActions`) per `scratch/explore.html` Round 5 (V4).**
   - Demote Auto-Ship from a permanent hero button to a compact physical `Auto` toggle switch (Figma UI3 style), promoting **Pause** and **Complete** to first-class controls with Notion-style soft, borderless chrome.
   - Use one unified Graph vocabulary in the status/action pill (`Running · <step>` / `Paused at <step>` / `Waiting · <step>` / `Merged · <step>`, with direct step picks inside the pill instead of overlapping `Moving` / `Run` / `Start` / `Start from…` words).
   - Keep left Graph and right controls non-duplicative (tie running/paused state to the active step without repeating a long sentence) and keep right-hand control slots fixed-width so clicking Pause/Run/Auto causes **0px horizontal jump**. Mirror cleanly to iPhone.

See `scratch/demo-notes.md` for exact quotes and `scratch/explore.html` for the interactive Round 5 V4 prototype.

## Done when

During a build, `lf task pause INF-123` stops the worker, every subprocess in the worker's OS session, and scheduled worker/limit retries. The Task keeps its owner, Machine, dirty checkout, PR, Graph position and history, and its primary Task Session stays open and usable. Restarting the Machine server, running `wave-operate`, or receiving a comment/steer starts no Graph run. `lf task unpause INF-123 [--run]` lifts the hold. On Mac and iPhone the Task stays in the Working list, in its **Paused** group.

## What exists

- **Hold.** `task_pauses(task_id PK, paused_at, paused_by, stopping_sessions)` on the execution Machine; `TaskState::Paused` derived in `sqlite/durable.rs::task_state_sql`. Pause applies only to open, started Tasks. Unpause, complete, abandon, reopen and unassign clear it; unassign refuses while `stopping_sessions` has live members. `lf task move` carries the hold in `MovedTask`; the source keeps it until arrival.
- **Stop.** Headless workers get their own OS session; pause, interrupt and the reaper stop its verified members, including reparented jobs. Pause records the hold, the session members and a Graph stop in one transaction, then signals outside the admission and SQLite locks; a separate per-Task cleanup lock serializes cleanup with unpause and move send. In-flight mutations (`lf land`) drain; the receipt and `TaskGraph.sentence` name them.
- **Gates.** `lf task run`, `lf task auto on`, carrier and limit recovery, and operator recommendations (`TaskActionModel` NoAction) respect the hold. Conversations do not: steers, child-Task updates and primary recovery reach the open Task Session as usual.
- **Surfaces.** `lf task pause`, `lf task unpause [--run]`, `lf task list --state paused`, served `task.pause` / `task.unpause` routed to the execution Machine, `TaskRunControl` refusal and `pausing` flag, Swift DTOs and `TaskActions.pause` (Unpause on Mac's Workflow bar and iPhone's action menu). Docs: `docs/work.md`, `surfaces.md`, `orchestration.md`, `sessions.md`, `troubleshooting.md`, `agent-api.md`.
- **Working.** Owned held Tasks stay in Working. Roadmap rows and view contents carry the same `working_group` from Rust. Active, Shipping and Paused keep their saved order, with the Mac status sort inside each group. The CLI prints the group; the iPhone Working feed shows it within the repository.
- **Verify.** New code and feature Graphs end with automatic verify after deliver. Previously captured Graphs keep their own steps. A post-merge check that fails gets a linked fix-forward Task.

## Working groups on a private Machine

- With FIX-2 in auto, `lf task pause FIX-1` left two Working rows. Both `lf task list --json` and the phone's `kind:task state:working` query returned FIX-1 as paused and FIX-2 as shipping. `lf wave show Product` kept a single Working section. Plain unpause returned FIX-1 to active.
- The branch CLI loaded both corrected builtin Graphs. Code said “You take part at ship-review.” Feature said “You take part at explore and ship-review.”
- At 10,000 Tasks with 30,520-byte captured Graphs, SQLite fetched the 100 Working Graphs by primary key in 1.18 ms (median of 20). With all 100 at start, a complete private `lf desktop view show` went from 1,450 ms to 427 ms by parsing each distinct command once per read (median of three debug CLI processes). All 100 after review read Shipping.
- On macOS the focused checks covered refusal, paused conversations, Working counts, group order and keyboard navigation. On an iOS 26.2 Simulator the three pause tests passed, including opening the live terminal from Working and restoring the held row when unpause was refused.
- Rust checks covered Paused taking precedence over auto, a running Graph crossing review into Shipping, selected and undecided XOR paths, and a served pause on another Machine. The DTO, schema, documented-command and golden-prompt checks passed. So did `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, the architecture check and the Swift module check.

- QA with real Claude workers (`scratch/qa.html`): pausing one of two live builds left the primary conversation running and the neighbor building; the sidebar snapshot showed Active / Shipping / Paused; a steer reached the paused Task's open conversation. Fixed: `task_operate` gained a `paused` disposition after the primary called a paused Task idle and offered `lf task run`. Before the concept-review fixes, the stopped run read "process exited with status 130 … launch fresh work" (same as after `lf task interrupt`).

## Concept-review fixes for the Working-groups demo

- A clean interrupt leaves execution idle. Plain unpause lifts the hold at the saved cursor, and Run continues from it. A failure recorded before pause stays visible. A carrier recovery that read the Task before pause checks its eligibility under run admission before counting or launching a restart.
- `state:paused` selects both owned and unassigned held Tasks. The primary conversation can report Blocked and its question, Error, Busy or Available in the Paused group. With no report, the row shows Paused. The Mac context menu and workflow bar use the same Pause/Unpause control.
- With auto off, a Task waiting at land stays Active. A Graph with no landing step stays Active unless auto is on. Code and feature can ship after ship-review, through verify and end. Paused still takes precedence over auto.
- Ten focused Rust checks passed on macOS, including a two-worker CLI run, plain interrupt, retained failure, carrier recovery, Group classification, the view query, and the selection-state fixture. On a private Machine the CLI query returned FIX-1 with `["paused","backlog"]` and FIX-2 with `["open","paused","working"]`. Plain unpause removed only FIX-2 from the query.
- The final Rust pass also covered a failed Graph following an interrupted step, served pause on a second Machine, and four command documentation checks. Five headless Mac tests passed for the menu, Working and Backlog rows, the five conversation reports, optimistic pause/refusal, and the shared DTO. On an iPhone Simulator, a refused unpause restored Paused in the Working feed. Formatting, Clippy with warnings denied, and the architecture map passed.
- On an in-memory SQLite database with 10,000 Tasks and 186 auto marks, the auto lookup used the task_auto primary key. The median was 0.092 ms for 100 requested IDs and 2.25 ms for all 10,000, including conversion to Python rows.

## Remaining, in order

1. **Demo the Working groups.** Pause one of two owned Tasks with the other in auto. Show Active / Shipping / Paused, the paused Task’s live conversation, plain unpause, and automatic verify after deliver.
2. **Task bar (revision 4).** After the Working-groups demo, follow `scratch/explore.html` Round 5 V4.
3. **Named gaps for the PR.** Separate-session Codex 0.171.0 tool jobs can survive. A pause after a move’s payload is sent can require a retry on arrival.
4. **Gate / CI.** Five-harness worker matrix, Linux session enumeration, lifeline/attachment/hosted-terminal suites.

## QA on a private Machine

- Two real Claude workers each ran a real `cargo build`. Pausing one stopped its worker, shell, `cargo`, `build-script-build` and primary in 2.3 s; the other build finished. A server restart and a comment left the held Task quiet, and `unpause --run` restarted the build in the same checkout (`scratch/qa.html`).
- Fixed during QA: the paused run refusal named `lf-<hex>` and the cleanup refusal named `task_<hex>`; both now use the short identifier. Status called a paused step Blocked ("launch fresh work"); it now reads "<step> stopped when the Task was paused".
- The Mac and iPhone Paused groups have had no live walkthrough yet; headless Swift and Simulator tests cover them (remaining step 1).

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

## Checks after simplifying pause

- `cargo nextest run` with `LOOPFLOW_TEST_LF="$PWD/target/debug/lf"` passed 13 focused macOS checks, including stopping one of two builds, crash recovery, competing PID ownership, wire fixtures, and documented commands. Five CLI fixtures had timed out building the executable inside the test deadline before using the prepared binary.
- `scripts/test_desktop.sh --skip-build --filter 'PendingChangesTests|everyStateRustSendsDecodes|pausedTaskShowsInItsOwnListAndOpensOnUnpause'` passed 19 tests after building the Swift package with tests.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and the architecture and Swift module checks passed. Gate and CI cover the other harnesses and Linux.

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

- On macOS, `pause_cancels_a_scheduled_limit_retry` passed. A stopped step had a retry due in one hour. The retry queue was empty for that Task immediately after CLI pause and after plain unpause. In the same run, `pause_stops_owned_build_and_keeps_claim` and `board_looks_up_each_hold_by_task_on_a_ten_thousand_task_machine` passed. Gate and CI own the remaining harness and Linux checks.
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
