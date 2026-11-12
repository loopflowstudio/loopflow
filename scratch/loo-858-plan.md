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
- Code slice reported: task_graph_launch_tests pass after fixes; 7 Codex-native
  `context_launch_tests`/`session_cli_tests` and 2 lib `graph_tests` fail
  identically at 9d1792fbc5 (pre-existing); a lib-wide run showed 11
  timeout/lock-poison failures in `server::remote`/subscriptions/accounts
  that pass alone. Graph fixtures now put a Claude stand-in on process PATH,
  widening an existing env race between `graph_tests` and
  `task_graph_launch_tests` in one process.
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

## Concept review (2026-11-12): next implement direction

The model holds: a launch runs Here, Held or Detached (`transport::Place`),
and nothing in this build is an owner process. What is left are names that
still say "owner" or "session" where the thing is a held terminal or a plain
detached lf. Rename, no behavior change:

- `os_process::start_lf_session` starts no Session: it is the detached
  launcher. Move it into `os_process::detached` as `start_lf` (with
  `session_environment` and `lf_session_shell_command`, renamed for what they
  build). Callers: `engine/launch.rs`, `ops/pr_landing.rs` (its `let owner =`
  becomes the launched Process), `ops/task/ship.rs`.
- Host-as-owner wording where the host is a held terminal:
  `transport/terminal.rs` module doc and tests ("the owner's terminal" → the
  held terminal), `transport/snapshot.rs` (`owner: &TerminalId` →
  `terminal`), `os_process/foreground.rs` ("when an owner holds the launch"
  → inside a held terminal), `transport/hosted.rs` log "planning sync
  unavailable to this owner".
- `transport/wire.rs` `Controlled.ready` / `Reply::Controlled.ready` stay
  optional (a viewer can meet a holder an earlier build started, since held
  terminals survive server replacement), but the comments say "a holder from
  an earlier build", not "retained owners".

Stays as is: viewer *ownership* (`Ownership`, `Event::Owner`, size/colour
owner, input token) is the control model, a different concept that goes with
the control-model Task, and so do Swift `OwnerProtocol`/`OwnerStream` and the
`tests/fixtures/dto/owner/` directory Swift reads. The `lf session serve`
mentions that remain all name an earlier build's process (restart path,
inventory, troubleshooting, shell store); they are correct.

No new product choice. `scratch/questions.md` assumptions stand: Desktop
control is macOS-only, so held shells needing the Machine server matter only
for remote Linux shells, which already run managed launches there.
