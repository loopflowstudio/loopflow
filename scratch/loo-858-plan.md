# LOO-858: headless work without a host

Steers (Jack Heart, 2026-11-12), word for word where it matters:

- "Strong bias for deleting `lf session serve`: the default is to remove it,
  with its tests, docs and install/rollover handling, and move Desktop shells
  to the holder. Keep a piece only if something concrete still breaks without
  it, and name that in the PR."
- Fold in LOO-830 (shells → holder, delete the owner path) and LOO-838
  (delete `--moment task-updates`); "when this PR lands, complete LOO-830 and
  LOO-838 alongside LOO-858."
- Architecture pass: `docs/dev/index.md` "What runs on a Machine" is the
  model ("Do not rewrite it; if the code contradicts a sentence, fix the
  sentence and say which in scratch/"). Make server.md, server-first.md,
  reference.md (LfProcess, Client API server, Desktop shell rows), machines.md
  "Process topology" and the transport README agree and link to it; the
  transport README "starts from held terminals, not from the owner";
  server-first.md keeps only telemetry, unbuilt pieces and open questions.
  Work `scratch/simplification-opportunities.md` in order 2 → 1 → 3; the
  takeover model is not this Task.
- Owners an earlier build left running: "not necessary any more. ideally it
  would just restart the session". So delete the viewer half too (no
  `Host::Owner`, no `lf session attach OWNER`, no `session.attach
  {owner_lf_process_id}`, no frame-counted stream, per-owner sockets or owner
  reconnect). `lf resume` of an owner-hosted Session: end the owner by its
  verified pid and birth (as retirement does, never by command pattern), wait
  for exit, resume held from provider history. Unverifiable or not exiting:
  start nothing, say it still runs under an earlier build, name its pid,
  stop. Already gone: resume held. Desktop's pane open takes the same path.
  Nothing sweeps unresumed owners. One test: a Session recorded with an owner
  transport and a live stand-in resumes held and the stand-in is gone. Update
  docs/sessions.md, troubleshooting and index.md's paragraph to say resume
  restarts them.

## Done (commits on this branch)

- `os_process::detached` (launcher pipe: Process ID on admission, `released`
  on let-go); `start_lf_session` uses it, output → `logs/detached.log`.
- `transport::delegate` picks `Place::{Held, Detached}` or runs here; bare
  relayed launches (Desktop pane, framed view) are held with bare hooks;
  headless Graph steps run here. `Holder::Owner` deleted. `transport::start`
  (the owner spawner) deleted: nothing in this build starts an owner.
- Desktop shells held (LOO-830): migration `held_desktop_shells.sql`
  (`terminal_id`, `launcher_lf_process_id`); Close and retirement send the
  held terminal a close notice; `Hosted::end_jobs` kills the shell session's
  leftovers off-loop and sets `jobs_settled`. Owner-side shell code (Close
  polling in `lifetime.rs`, `shell_should_close`, `shell_holds_work`,
  `LF_DESKTOP_SHELL`, `Local.shell_jobs`) deleted.
- `lf process rollover` deleted; inventory lives in `lf/commands/binaries.rs`.
- LOO-838: `ContextMoment::TaskUpdates` gone; a parse error in any `__` hook
  exits 0 with the "earlier lf" notice.

- Compress: attach reply drops `reconnect` and `grace_ms` (Rust, Swift
  `OwnerProtocol`, `owner/observer.bin` regenerated); Swift drops the
  never-sent `reconnect_required`. `SessionConnectionProofTests` opens the pane
  on `lf -i -a claude : proof` (relayed, so held) instead of the deleted
  `session serve` owner. `.lf/wave/infrastructure/MEMORY.md` still says
  exhaustion answers `reconnect_required` (Wave memory is read-only here).

- Opportunity 1 (a host is a TerminalId): `lf session serve`, `lf session
  attach OWNER`, owner `session.attach`, `transport::Host`, `Spool`, viewer
  resume, owner reconnect, `hosted::Child`, store `Transport` and the
  DELEGATED CTE deleted. `ops::human_session::end_earlier_owner` restarts a
  Session an earlier build's owner runs (verified pid+birth, 10 s, else names
  the pid and starts nothing); test
  `resume_ends_an_earlier_builds_owner_and_continues_held`. Swift
  `ServerProtocol` follows (`sessionAttach(session:)`, `bridged(terminalID:)`).
  Columns `transport_lf_process_id`, `processes.viewer_hold*` stay (staged
  rule).

## Remaining

- File the contract-migration Task for `agent_sessions.transport_lf_process_id`
  and `processes.viewer_hold*` once the staging window passes.
- Coverage deleted with no held equivalent:
  `context_launch_tests::codex_service_tier_survives_live_reattachment` (held
  bare Codex has no app-server),
  `process_ownership_tests::interruption_records_the_process_without_a_fabricated_signal_name`,
  `graph_tests::ordinary_graph_crash_retains_effects…` (no non-agent blocking
  Graph step without serve).
  Owner-backed server tests deleted in `64f2b3697f`
  (`server_reconnect_*`, `server_move_here_*`,
  `server_failed_setup_retry_only_reconnects_and_preserves_newer_claims`,
  `server_preserves_owner_and_graph_across_restart`,
  `installed_server_preserves_owner_and_graph_across_replacement`): held cases
  cover restart (`held_session_survives_server_restart`), reconnect
  (`a_held_pane_reads_reconnecting_until_the_next_server_answers`) and
  ownership (`ownership_queues_behind_a_taker_and_tells_the_loser_who_took_it`).
  Move here with a lost ack, failed-setup retry and installed-server
  replacement have no held case; the control-model Task is where they return.

## Checks

- Compress: clippy --all-targets clean; `--lib transport::` 70 pass;
  transport_tests 42 pass / 5 ignored; `swift test --filter OwnerStreamTests`
  7 pass. `SessionConnectionProofTests` (.requiresDisplay) not run: gate.

- transport_tests 42 pass / 5 ignored; session_lifecycle_tests 41 pass;
  documented_commands + cli_discovery 40 pass; fmt and clippy --all-targets
  clean (after Opportunity 1).
- Code slice reported 7 Codex-native `context_launch_tests`/`session_cli_tests`
  and 2 lib `graph_tests` failing "pre-existing". Gate (2026-11-13) found they
  pass at the base dfbea8cf81 and fail here: they reached an owner-hosted
  Codex through a piped `-i` launch, which now runs here as plain `codex`.
  Fixed in the tests (see Gate below).
- Not run here: `ssh_shells::` (ignored on macOS), slow
  `native_runner_is_quiet_until_its_turn_ends_codex_foreground`, Swift/iOS
  suites. Gate/CI.
- ~38 `com.loopflow.server.<hash>` launchd jobs point at deleted
  `/private/tmp/lf-7501-*` homes; ownership unverified, left alone.

## Docs

`docs/dev/index.md` "What runs on a Machine" sentences changed because the
code contradicts them:

- Place table, Held "What" cell: added "a bare Session a Desktop pane or
  framed `lf --machine` view opens". `transport::delegate` holds bare relayed
  launches (with the two bare hooks); the table listed only Work and shells.
- "Work decides between Here and the other two": now names the exception
  (a Desktop pane or framed `lf --machine` view holds a bare Session too), for
  the same reason.
- "Conversations started by a build from before held terminals…": now says
  `lf resume` / Desktop's pane open restarts them (verified pid and birth,
  wait, resume held; unverifiable or not exiting keeps running and names its
  pid), nothing sweeps them. Per the steer above.

The owner-protocol docs above now match the tree (no `owner_lf_process_id`,
spool or `resume {received}` left in docs). `rust/loopflow/tests/README.md`'s
Client API server list names held tests in place of the deleted owner ones.

## Concept review (2026-11-12): renames, done

A launch runs Here, Held or Detached (`transport::Place`); nothing in this
build is an owner process. The renames are in the tree, no behavior change:
`os_process::detached::start_lf` (with `launch_environment` and
`shell_command`) replaces `start_lf_session`; host-as-owner wording in
`transport/{terminal,output,snapshot,hosted,wire}.rs`, `os_process/foreground.rs`,
`engine/launch.rs`, `session.rs` and `bin/lf.rs` says held terminal or detached
process; compress finished the sweep in `transport/{native,input,stream}.rs`,
`ghostty_vt.rs`, `context_block.rs`, `server/{remote,service}.rs`,
`ops/human_session.rs`. "Owner protocol"/"owner stream" (the wire Swift's
`OwnerProtocol` speaks) and `desktop_shells.owner` (a real earlier-build
column) keep the word. `Controlled.ready` stays optional: a viewer can meet a holder from an
earlier build.

Stays as is: viewer *ownership* (`Ownership`, `Event::Owner`, size/colour
owner, input token) is the control model and goes with the control-model Task,
as do Swift `OwnerProtocol`/`OwnerStream` and `tests/fixtures/dto/owner/`.
Also left: `harness/native_exit.rs`'s `AttachmentOwner` and its "execution
owner" module doc (a type rename, not wording), CI repair "owner" in
`ops/pr_landing.rs`, and `lf open`'s Machine "execution owner".

Checks (2026-11-12): clippy `--all-targets` clean; `--lib transport::
os_process:: engine::launch::` 114 passed; `transport_tests` 42 passed, 5
ignored; `server::schema::tests::` 7 passed. Gate owns the rest.
Compress: clippy `--all-targets` clean; `--lib transport::` 70 passed;
`transport_tests` 42 passed, 5 ignored.

## Concept review, second pass (2026-11-12): nothing to change

Every remaining `lf session serve` mention in code and docs names an
earlier build's process: Session owners restart on `lf resume`, an earlier
Desktop shell reads "Shell ended. Create a new shell" (a plain shell has no
history to continue, the same restart rule). `transport::Place` is
`{Held, Detached}` plus run-here; no second launch path is left. The
`questions.md` entries follow from the steers (shells → holder implies the
Machine server). No open product choice.

## QA (2026-11-12)

Walkthrough in `scratch/qa.html`. Fixed: docs claimed a restarted earlier-build
owner always resumes held (a bare `lf resume` runs in its terminal; see
questions.md); `end_earlier_owner` now shows "Ending the earlier lf (pid N)…"
as the startup stage before its up-to-10 s wait. Checks: fmt, clippy
`--all-targets` clean; `resume_ends_an_earlier_builds_owner_and_continues_held`
passes.

## Sync check (2026-11-12)

- `cargo test -p loopflow --lib -- store::sqlite::task_retirement::tests:: store::query_plans::tests::` (LF variables unset, stdin closed): compiled after converting main's query-plan fixture to held terminals; linker warned about the large `__eh_frame` section. The test process stalled before main (`sample`: `_dyld_start`, 112K footprint, no test output) and was terminated with SIGTERM. Gate/CI must run these tests; no assertions ran.

## Concept review with Jack (2026-11-13): open

Page: `scratch/concept-review.html`. Monitoring follow-up is LOO-879 (product
Wave, `feature` graph), whose brief starts with this review.

Jack Heart, 2026-11-13: "to be clear, i want to concept-reivew this diff so i
can review."

Unanswered, numbered as on the page:

1. Upgrading with conversations running: as built (quit and `lf resume` each),
   a bulk "end idle conversations on superseded builds" before landing, or
   that action inside LOO-879. Seen: `lf ps` lists live processes on 17
   superseded builds on this Machine.
2. `-b` Work commands outlive their caller (as built), or only Graphs,
   landing watchers and `lf ship` detach.
3. An earlier build's bare conversation resumed from a terminal: restarts in
   that terminal (as built) or held.
4. "Detached" as both a Session state and a place: leave, or rename the place.
5. Desktop shells: Close ends at once and shells need the Machine server.

## Review direction, Jack Heart, 2026-11-13 (not yet implemented)

On "Start a Task's Graph … Returns after one line naming the Graph and its
log; the Graph is a plain process": "hmm i dont think i like this".

"specifically what i am concerned about is:
- no watching backgroudn runs for headless graphs skills. this nice to have
  but i dont see why we cant have it
- when you have interactive steps in a graph, they should be have like a
  normal bare lf interactive skil"

"i dont want it to be held, i just want it to be bare if its coming from a
terminal / non held env"

As built: `transport::delegate` holds any non-`-b` Work launch at a terminal,
so an interactive step of a Task's Graph (`lf -i task run`, `lf --task X graph
G`, an interactive `lf ship` step) opens in a new held terminal behind an `lf`
relay, and `lf task run` typed at a terminal is itself held before it lets its
launcher go. Nothing follows a Graph already running; only `lf -b task run`
prints the log, from the start.

Jack Heart, 2026-11-13, on whether such a step keeps Work hooks: "the way
skills are supposed to work is that if you do lf task run ... its held, but
even from a task worktree, lf <skill> is not and do not get task update hooks.
graphs should be the same."

So the command decides, for Graphs as for Skills: `lf task run` is held Work;
`lf graph <g>`, like `lf <skill>`, is bare even in a Task's worktree: its
steps run here, interactive ones as the terminal's foreground agent, with the
two bare hooks and no Task updates. `transport::delegate` already places a
bare Graph and its steps here; no placement change follows from this. The
earlier idea of running a Task run's interactive step in the caller's
terminal is withdrawn. Worth one check before landing: a bare `lf graph` with
an interactive step, run from a Task's worktree, opens that step as the
terminal's own agent with bare hooks.

Still wanted:
- Watching is following: a Graph's log is the view. `lf task run` at a
  terminal follows it and Ctrl-C lets go; one command follows a Graph already
  running. No PTY, no host.

## Gate (2026-11-13)

Runner: `scripts/test.py --reuse-passing --rust-filter …` stopped at its
resource preflight ("cannot measure ~/.cache/uv: 15-second inventory deadline
exceeded", 44 GiB free); its plan ran by hand. Machine load1 was ~480 from
other checkouts throughout, so timing-bound fixtures failed under load.
Failures were rerun one by one from the built binaries (empty cwd, temp
`HOME`/`LF_HOME`, PATH with no harnesses) and compared against a build of
the base dfbea8cf81.

Branch regressions, fixed in tests (the product change is the
`questions.md` piped-launch entry):
- `native_codex.py` recorded a launch only on its `--remote` client path; a
  bare launch is now plain `codex`, so it records and prints there too.
  Fixes `graph_tests::{checkout_task_identity_ignores_main_and_parent_upstreams,
  lf_launches_inside_a_task_checkout_bind_to_that_task}` and
  `session_cli_tests::resume_admits_native_claude_and_codex_ids_and_keeps_their_identity`.
- `context_launch_tests::{codex_terminal_delivers_large_input…,
  codex_service_tier_reaches_native_and_headless_launch_and_resume,
  codex_startup_failures_preserve_input_and_allow_reconnect,
  explicit_documents_match_preview_capture_and_codex_input}`: the interactive
  launch names a Task (`codex_task`: Machine server, adopted linked checkout,
  so no Task conversation starts beside it and shares the stand-in's files).
  Service tier's `-i` resume probe starts a separate thread, so the
  app-server rightly outlives the exit (`codex_connection.rs`: "A separately
  started conversation must survive this exit"); the test asserts that one
  survivor and kills it. `codex_startup_failures…` fails at base and passes now.
- `session_cli_tests::provider_switch_stops_startup_output_before_codex_opens`:
  the ready marker fires on any terminal launch, not only `--remote`.
- `task_graph_launch_tests::running_graphs_belong_to_the_target_checkout_and_retain_their_caller`
  (3 of 4 failed): the Task's background conversation runs the same Claude
  stand-in and can write `ready` before the Graph's step; the test now waits
  for the Graph process. 4 of 4 pass.

Fail at base too, left: `graph_tests::interactive_graph_keeps_input…` (30 s
deadline; base and branch both take ~35 s to the first step at load ~480),
`journal::tests::a_held_write_lock_delays_an_process_by_one_receipt_wait`,
`session_cli_tests::terminal_titles_follow_session_rename…`,
`global_commands::open_delivers_exact_task_to_desktop…` ("No agent selected"
with zero or several harnesses on PATH, base too).

Load-only (pass alone): `server::remote::tests::only_owner_identity…`,
`transport::registry::tests::a_view_continues…`, three
`transport::viewer_tests::snapshot_preview_*`, three `task_planning_tests`.
`planning_export_late_readback_preserves_newer_edits_and_inventory_identity`
parked in `block_on` for 65 min in the full run (killed, pid verified as
this run's child); passes alone. `explicit_documents_…` hit "Native dispatch
timed out" in 2 of 4 runs under load.

Main's bare Codex (#1895) puts the first turn and instructions in argv
(`-c developer_instructions=…`); a 480 KB skill exceeds Linux's 128 KB
per-argument limit. Not this branch; the tests that sent large input now go
through Work.
