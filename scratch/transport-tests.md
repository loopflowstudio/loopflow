# LOO-887: rebuild the transport tests around the current design

Built on PR #1920 (`be8ffd616a`, on main and merged here): its seeded
fixture homes, host-level grace test and three-cycle shell test. Its notes and
CI timings: `git show c4e1251328:scratch/test-speed.md` and
`…:scratch/test-timings.tsv`.

## Outcome and demo

Transport rules run in-process in well under 100 ms each; a short list of
real-process tests proves each surface is wired and cleaned up. Demo:

```sh
cargo nextest run -p loopflow --lib -E 'test(transport::)'          # host + relay rules, seconds
cargo nextest run -p loopflow --test transport_tests                # ~30 real-process tests
```

Before (CI medians from the parent's `test-timings.tsv`, three merge-queue runs): 131 tests
in `transport_tests`, 1,646 s summed, 70 over 10 s. Lib `transport::` tests: 72
tests, ~117 s (registry 63 s, viewer 38 s, lifetime 14 s).

Done when:
- `transport_tests` holds at most 35 tests and sums under 400 s on CI (Linux),
  with no test waiting out a production timeout (30 s heartbeat, 60 s grace,
  5 s attach, a full 12.7 s reconnect cycle). No fallback was needed
  (slice 2 reached the fake server without a seam). One kept test still waits a
  production interval on purpose: `server_mobile_changes_…`, the mobile
  manager's 5 s retry after a Tailscale outage.
- Every rule a deleted real-process test asserted is asserted by a named
  in-process test (table below) or deliberately dropped with a reason.
- New host-rule tests each run under 100 ms; relay-rule cases under 1 s,
  except five that must span a window (setup, unacknowledged, outage, move,
  quiet: 1.5–3.4 s under load), kept at those times in slice 6.
- No production signature, trait or parameter exists only for tests;
  timing overrides are `#[cfg(test)]` only (the `server::holder::TEST_IDLE`
  pattern).

## What the code is now (constrains the design)

- `transport::delegate` (`src/transport.rs:360`) picks the host: Work viewed at a
  terminal → held terminal (`start_held`, server + holder); bare launch at a
  terminal → no host, foreground; piped, `-b`, Desktop-surface non-Work, Desktop
  Task shells → owner (`lf session serve`). Owners are live code, not legacy.
  README line ~87 still says bare Graphs keep their host; the code runs them in
  the foreground. Fix the README in this Task.
- `transport::hosted::Hosted` is the viewer-protocol host for **both** owners
  (`transport.rs:1234`) and held terminals (`registry.rs:1003`). A test against
  `Hosted::pass` runs the same code as both real paths. This is the seam the
  Task brief asks for.
- Owner-side time is already data: `Viewer.heartbeat_at`, `Connection::Dropped(at)`,
  `Hosted.exit = (code, at)`, `Viewer::expire_heartbeat(now)` (`transport.rs:818`),
  `viewer_hold(.., now)` (`:1319`). Backdating a field is the time seam; the
  parent branch's `hosted::tests::grace_expiry_refuses_bytes_before_repainting_the_terminal_screen`
  (62 s → 0.025 s) is the template.
- Relay-side time is not data: `view()` (`transport.rs:2070`, ~540 lines) keeps
  `heartbeat`/`last_response` locals; `connection::Status` backoff is
  `100ms << min(n,5)` × 8 attempts ≈ 12.7 s (`connection.rs:14,275`).
  `relay_tests.rs` runs the relay by re-executing the lib test binary on a PTY
  against an in-test fake host socket.
- `server::holder::TEST_IDLE` (`server/holder.rs:41-46`) is the precedent for a
  `#[cfg(test)]` thread-local timing value read where the constant is used.

## Findings from this kickoff

- **The 10.5 s cluster is not the fixture.** Timing probe in
  `end_recorded_processes` (Mac, debug, 2026-11-13): no recorded processes left
  at drop, both phases < 25 µs. `prepared_reveal_preserves_a_later_controlling_viewer`
  4.993 s, `prepared_start_preserves_a_later_controlling_viewer` 1.221 s locally
  vs 10.60 / 10.49 s CI medians. The cause is Linux/CI-only and unlocated.
  Candidates: `Hosted::deadline`'s 10 s fallback wake (`hosted.rs:1188`) if a
  wake is missed on Linux, `HEARTBEAT_INTERVAL` 10 s, holder `ANSWER` 10 s.
  Most cluster tests are deleted or moved below, so only measure it if a kept
  real-process test still shows ~10 s on CI.
- Simple owner tests cost ~5–6 s: an owner lingers 2 s after its child exits
  (`hosted.rs:1171`) plus process startup. Accept it for the kept set.
- `ghostty_vt`'s 7 unit tests also run in `transport_tests` through its `#[path]`
  include. Negligible; left alone.
- `scripts/testing.md` routes store-wide schema changes to `binary(transport_tests)`
  because owner lifetimes exercise store writers. After the rebuild, point that
  line at the kept real-process tests (or `test(transport::)` + the store suites)
  so it still means something.

## Design

Three layers. Each rule lives in exactly one.

### 1. Host rules: `Hosted::pass` in-process (`src/transport/host_tests.rs`)

A `#[cfg(test)]` module included from `hosted.rs` (like `registry_tests.rs`).
One small bench: `Hosted::new(Host::Owner(..))`, `Child::Local` over pipes (the
test writes program output into a reader pipe, reads program input from the
input pipe, sends the exit on the `mpsc` channel), viewers as `UnixStream`
pairs admitted with `host.admit`, a peer helper that sends `Request`s and
decodes frames, and `pass()` driven by the test. Time moves by backdating
viewer and exit fields. `TestLedgerGuard` for the store.

Rules that move here: attach/Busy/replace, takeover notices and queueing,
ownership (size/theme) arbitration, snapshot at the viewer's size, resume from
byte count, spool overflow → `reconnect_required` → repaint, resume replacing
its own socket without taking newer claims, heartbeat expiry of only the silent
viewer, grace expiry, viewer holds and absence start, input surviving a viewer
disconnect, input with a locked store, EOF-before-claims ordering, event
nonce/seq and resize events, slow observer cut at `LIMIT`, input hints only to
controllers, directory events, `Controlled{ready}` readiness, wire overhead of a
1 MiB burst (pure `wire.rs` test).

### 2. Relay rules: re-exec'd lib test binary on a PTY (`relay_tests.rs`)

The relay (`view`, `relay_over`, `connection::Status`) runs in a child of the
lib test binary on a real PTY, against a fake host socket in the parent test
(a bench from layer 1 on a thread where the rule needs a real host). Add a
`#[cfg(test)]` thread-local timing set (heartbeat interval/timeout, attach
timeout, reconnect backoff) read where the constants are used; the child's
fixture sets it before relaying. Non-test builds keep the constants.

Rules that move here: bridge stops answering → reconnect; quiet host keeps the
relay attached across heartbeats; attach acknowledgement timeout and
cancellation keep the execution; acknowledged viewer survives past the attach
deadline; a host that stops reading input ends the viewer with a cause;
control handoff restores the terminal and prints the notice; typing during an
outage is discarded, never replayed; reconnect exhaustion reports unavailable,
Retry reconnects without reaching the provider; pane closure cancels a pending
move/reconnect/installer; Move here consumes before sending.

Risk: `connection::Status` opens the server bridge itself. If a fake server
socket cannot stand in without a production seam, keep the server-bridge
heartbeat and exhaustion cases as **one** real-process test each (merged; one
30 s window, one 12.7 s cycle) and say so here.

### 3. Real-process surface tests (`tests/transport_tests.rs`, 34 + 8 ignored)

What exists after slice 4 (merged legs keep their own `Fixture`):

| Surface | Kept tests |
|---|---|
| Owner wiring | `every_viewer_starts_with_the_snapshot_and_shares_live_output` (+ directories), `detached_owner_keeps_no_descriptor_its_terminal_inherited`, `owned_program_does_not_inherit_the_desktop_pane_relay`, `pipe_viewers_deliver_input_size_and_exit` |
| Bare launch | `bare_lf_runs_its_provider_in_the_foreground_and_ends_with_its_terminal`, `first_native_launch_keeps_only_valid_desktop_terminal_placement` |
| Launcher → provider | `a_terminal_launcher_becomes_its_provider_and_the_owner_records_its_exit` (+ failed provider, Graph exit 1, no `lf` children), `native_pending_request_survives_creator_exit_…`, `codex_original_request_channel_and_turn_survive_all_viewers`, `interactive_graph_settles_each_step_before_starting_the_next` |
| Headless | `headless_runs_settle_their_original_step_after_creator_teardown` (graph + skill) |
| CLI open/JSON | `conversation_open_here_confirms_attachment_…`, `prepared_start_preserves_a_claim_time_winner`, `resume_json_returns_after_relay_and_viewer_takeover`, `json_launches_start_once_and_return_one_record` |
| Build skew / takeover | `resume_takes_over_an_owner_from_another_build`, `resume_takes_over_only_an_owner_that_does_not_answer`, `a_held_view_meeting_another_build_reopens_as_the_installed_lf` |
| Local relay | `local_terminal_reconnects_without_replaying_input_or_taking_newer_claims` |
| Server relay | `server_preserves_owner_and_graph_across_restart` (+ Move here), `server_reconnect_preserves_newer_viewers_input_and_geometry`, `a_stalled_bridge_leaves_the_healthy_bridge_…`, `server_startup_cancels_its_pending_installer_…`, `server_status_does_not_install_or_create_work`, `server_mobile_changes_preserve_local_connections` |
| Held terminals | `a_held_pane_reads_reconnecting_…`, `held_session_survives_server_restart`, `a_held_view_waits_for_a_holder_stopped_at_server_startup`, `a_launch_refused_for_a_stopped_server_…`, `held_terminals_share_one_holder_and_add_no_server_threads`, `task_cleanup_ends_an_abandoned_tasks_held_terminal`, `a_held_conversation_outlives_its_view` |
| Desktop shell | `desktop_shell_reconnects_without_replay_and_close_settles_its_jobs`, `completed_task_cleanup_signals_shell_owner_and_settles_jobs` |
| SSH (`transport/ssh_shells.rs`, Linux CI) | `ssh_shell_lifecycles_…` (2 cycles), `ssh_shell_authentication_failure_…`, `ssh_prepared_viewer_…` |
| Ignored acceptance | Rex ×3, mobile native ×3, launchd registration ×2, untouched |

### Moves out of the transport suite (not transport rules)

| Rule | Destination |
|---|---|
| superseded-owner retention (5 `unanswering_superseded_owners_preserve_*`) | store `owner_retention` table test (`steers_tests.rs`), plus `human_session::tests::a_superseded_owner_keeps_a_live_turn_…` for `attaches` (slice 5) |
| prepare reads `detached` (`opening_reattaches_an_owner_whose_viewer_record_went_stale`, prepare half of `opening_takes_over_…`) | `human_session::viewed_summary` test with a fake socket at `transport::endpoint` |
| native settlement vs cleanup (`silent_native_turns_survive_until_settlement`, `native_background_watches_survive_until_settlement`, hook part of `retained_hooks_survive_upgrade`) | `server/boundary.rs`, `retirement_tests`, Clap alias parse test |
| `resume_refuses_a_graph_step_and_leaves_it_running` | `human_session::run_refusal` test |
| `conversation_open_continues_closed_history_without_replaying_the_request` | `ops/human_session/tests.rs` beside the reveal tests |
| `provider_changing_resume_json_launches_once_and_keeps_the_session` | in-process provider-switch history test; launch-once covered by the kept JSON test |
| `conversation_open_pending_retry_waits_for_the_controlling_viewers_readiness` | host rule (`Controlled{ready}`) + `open.rs` mapping test |
| server subscriptions, portfolio, mobile setup/changes, tickets, admission (`server_task_watch_…`, `server_subscriptions_…`, `server_mobile_setup_…`, config branches of `server_mobile_changes_…`, `an_attach_ticket_…`, `terminal_bridges_leave_the_client_connections_free`, `exact_owner_refuses_a_replacement_while_session_attach_finds_it`) | `server/service.rs`, `server/mobile.rs` tests (extend `attach_ticket_bridges_a_second_connection_to_the_owner`); `server_mobile_changes_…` keeps one real listener-restore test |
| `continuing_a_session_starts_one_held_terminal`, `a_held_terminal_with_no_view_reports_that_it_waits` | `service.rs` / `registry_tests` |
| `native_exit_preserves_returning_launcher_results` code→outcome matrix | `native_exit::settle` + `provider_exit_code` table with `ExitStatus::from_raw` |
| `completed_task_closes_a_conversation_resumed_after_checkout_retirement` decision | `retirement_tests` |
| `taskless_session_renames_…`, `native_program_status_has_no_stale_launcher_working_record` | `output.rs` / host bench |

### Delete — do not maintain

Duplicates or guards for removed mechanisms; each one's rule is covered above.
Slices 1–4 deleted the whole list; names are in
`git show 7acc5cea27 117378aadb da53314382 dc3d722f36 79f92459fc -- rust/loopflow/tests/transport_tests.rs`
and the slice 4 commit.
Unused fixture helpers go with them. `lifetime.rs` `POLL` is a `#[cfg(test)]` value (slice 6).

## Forbidden

No env var or CLI flag that shortens a production timeout; no factory trait or
clock trait; no second protocol host for tests (the bench is `Hosted` itself);
no test that waits out a production timeout except the one named above.

## Slices

1. **Done (2026-11-13).** `src/transport/host_tests.rs`: a `Bench` over
   `Hosted::pass` with the program on an in-process raw PTY or pipes and
   viewers on socket pairs; the bench owns its peers so it can drain them while
   passing. 21 host rules, 0.37 s wall locally (slow observer 0.26 s and the
   1 MiB wire test 0.12 s move 4 MiB/1 MiB; the rest 20–90 ms). The parent's
   grace test moved in. Deleted 22 real-process tests (184 s of CI medians);
   `transport_tests` now holds 105 tests. A mutation (`resume.control` ignoring a
   newer claim) fails `resume_supersedes_…` in the bench.
   Compressed: `Bench::new` builds its own host; `attached`/`resume` helpers
   replace eight hand-built resume requests. `cargo nextest run -p loopflow
   --lib -E 'test(transport::hosted::host_tests::)'`: 21/21, 0.252 s.
   Left from this slice:
   - Not ported: the bridge process exiting when its owner cuts it
     (`viewer_stream_backpressure_…`, a `stream.rs` rule); the hold released at
     exit (`release_viewer_hold` always computes from no viewers).
2. **Done (2026-11-13).** `src/transport/relay_tests.rs`: the child relays a
   Desktop pane to a held terminal (`Host::Terminal`) on a real PTY; its own
   threads play the server entrance (`registry::entrance()` under the child's
   `LF_HOME`) and the pane's status socket, and `LF_BIN` is an installer script.
   No production seam was needed: `connection::Status` reaches the fake through
   the entrance path. Relay time is `transport::Pace` (`PACE` in release,
   `#[cfg(test)]` `TEST_PACE` thread-local); `HEARTBEAT_INTERVAL` and
   `ATTACH_TIMEOUT` are gone, host-side `HEARTBEAT_TIMEOUT` stays. Six cases,
   0.5–3.1 s wall each on a loaded Mac (child startup ≈ 0.5 s; exhaustion and
   quiet-window cases ≈ 2 s because `Status::wait` polls in 50 ms steps). A
   mutation disabling the relay's silence check fails the quiet/silent case.
   Deleted 10 real-process tests, 218.5 s of CI medians; `transport_tests`
   holds 95. Compressed: single-use `Peer` helpers inlined into `Server::accept`,
   setup check shared. `cargo nextest run -p loopflow --lib -E
   'test(transport::relay_tests::)'`: 7/7 (six cases plus `relay_fixture`, the
   child's entry point), 2.83 s wall (0.7–2.8 s each).
   Gaps left on purpose:
   - Relay cases use only `Host::Terminal`; owner bridging through the server
     ticket (`bridge_existing`) stays covered by the kept
     `server_reconnect_preserves_newer_viewers_input_and_geometry` and
     `server_preserves_owner_and_graph_across_restart`.
   - Pane closure while a Move here is pending (relay SIGSTOPped) is not
     ported; the closure case covers a pending reconnect and `take_move`
     checks `closed()` before consuming.
3. **Done (2026-11-13).** 32 real-process tests deleted, 366.1 s of CI
   medians; `transport_tests` holds 63 `#[test]`s. In-process homes:
   - `store::sqlite::steers::tests::owner_retention_names_the_graph_run_or_absence_that_holds_an_idle_owner`
     (graph, step, run, completed; request and turn were already the
     `pending_requests_…` and `silent_native_turns_…` cases there).
   - `server::boundary::tests::a_turn_end_settles_only_once_no_watch_or_wakeup_remains`
     (Stop with background tasks, crons, or neither list).
   - `lf::tests::a_retained_stop_hook_spelling_reaches_the_turn_end` (`__planning-check`).
   - `harness::native_exit::tests::a_failed_provider_exit_carries_its_own_code_and_says_so`,
     now a table over `ExitStatus::from_raw` (3, 7, 130, SIGINT, SIGKILL).
     `native_exit_preserves_returning_launcher_results` keeps only the Graph
     exit-1 case; slice 4 merges it into the launcher test.
   - `ops::task::retirement::tests::conversation_cleanup_owns_grace_membership_and_work`
     gained the retired-checkout case.
   - `ops::human_session::tests::a_stale_viewer_record_reads_unviewed_unless_the_owner_says_it_is_held`
     (fake owner socket: held, not held, never answers, absent). 1.0 s: the
     never-answering case waits out `transport::attachment`'s 1 s read timeout.
     `opening_takes_over_…` lost its prepare half and keeps the takeover.
   - The typed-input screen assertion joined
     `completed_task_cleanup_signals_shell_owner_and_settles_jobs` (shell
     owners keep their screen under the owner id).
   Dropped with the rule covered elsewhere:
   - `resume_refuses_a_graph_step_…`: `human_session::tests::resume_refuses_a_run_…`
     covers the refusal; the Graph name in the label is `session_summary().graph`.
   - `conversation_open_continues_closed_history_…`: native `--resume` argv is
     `agent::session` tests; no replay is `automatic_start_…` and `reveal_requires_…`.
   - `provider_changing_resume_json_…`: same `new_thread` path as the kept JSON
     test; Codex history import is `conversation_log::tests::codex_log_…`.
   - `conversation_open_pending_retry_…`: readiness is the bench's
     `Controlled{ready}` rule; `open.rs:1094` maps the reply straight to
     Opening/Usable.
   - `taskless_session_renames_…`: `output.rs` title tests plus
     `session_cli_tests::terminal_titles_follow_session_rename_…`.
   - `native_program_status_has_no_stale_launcher_working_record`: the native
     launcher execs its provider and sends no Status; pass-through and replay
     are `output.rs`'s `program_status_*` tests.
   Gap: a subagent's Stop (`agent_id`) is ignored by `context.rs:251`, which
   reads the callback from stdin; no in-process test covers that guard.
   - Server rows: `server::service::tests::` `a_task_watch_follows_renames_…`
     (3.4 s: about 1 s waiting for the portfolio file's refresh),
     `a_plan_watch_refreshes_…`, `session_attach_tickets_the_current_owner_…`,
     `continuing_a_session_resumes_exactly_it_…`, and
     `attach_ticket_bridges_a_second_connection_to_the_owner` extended (72
     bridges leave client slots free; malformed attach); `server::mobile::tests::`
     `restoration_keeps_the_saved_address_…`, `offline_disable_keeps_the_approval_…`,
     `a_saved_approval_does_not_follow_a_new_tailscale_owner`;
     `registry_tests::a_held_terminal_with_no_view_reports_that_it_waits`.
     Store-opening tests cost 1.3–1.5 s each (`open_ephemeral` runs every
     migration). Not ported: the socket read matching `lf desktop view show
     --json` (both call one function).
   `server_mobile_changes_preserve_local_connections_and_saved_disable` stays
   real-process for listener restore, recovery after a Tailscale outage (the
   manager's 5 s retry), logout closing remote connections, refusing a foreign
   device and the `server.plist` check; its config branches are now redundant
   and go in slice 4.
   Compressed: the retention test's string-matched cases became a
   `retention_after(setup)` helper with one assertion each. `cargo nextest run
   -p loopflow --lib -E 'test(owner_retention_names)'`: pass, 0.18 s; clippy
   `--lib --tests --test transport_tests` clean (no orphaned helpers).
4. **Done (2026-11-13).** `transport_tests` holds 34 non-ignored `#[test]`s
   (42 with the 8 ignored acceptance tests; see `scratch/questions.md`).
   Local run, Mac, debug, load average ~650, `--test-threads 6`: 34/34 pass,
   166.4 s summed, 34.4 s wall; slowest `resume_takes_over_only_…` 16.3 s.
   - Ported to `relay_tests` (child relays a terminal to an owner socket a
     thread plays; `owner-…` cases set no `LF_SERVER_STATUS`): control handoff
     0.11 s, stalled owner 0.40 s, unacknowledged owner 0.64 s, acknowledged
     past the attach deadline 1.1 s. `Pace` gained `stall` (the 2 s
     `Stream::write` deadline). The quiet case now also asserts the pane reads
     connecting and takes no keys before acknowledgement, replacing
     `server_initial_connection_…`. Deleted the five real-process tests plus
     `SilentOwner`/`silent_attachment`/`accept_attachment`. Not ported: the
     SIGINT case (exit 130 is `lf`'s `ctrlc` handler).
   - Merged (legs keep their own fixture): launcher test ← failed provider,
     Graph exit 1, no `lf` children (from `opening_a_prepared_…`, deleted; its
     exec-keeps-Process check joined `resume_json_returns_after_relay_…`);
     `json_launches_start_once_and_return_one_record` ← three JSON tests;
     `resume_takes_over_only_an_owner_that_does_not_answer` ← three takeover
     tests; headless graph + skill; held placement into
     `a_held_conversation_outlives_its_view`; Move here into the restart test;
     `session.attach` waiting into `a_held_view_waits_…` (ticket-after-end leg
     dropped: the redemption uses `registry::refusal`, the same answer
     `registry_tests::an_ended_terminal_…` checks); server threads into
     `held_terminals_share_one_holder_and_add_no_server_threads`; directory
     into the snapshot test; both pipe viewers; the `getsid` sweep and a
     stale-checkout create into the Desktop shell test.
   - Sleeps removed: 6 s (ported), 5 s (now a hosted check right after the
     view stops; nothing in `lifetime.rs` ends an unviewed conversation), 4 s
     headless and 4 s shell (`shell_should_close` reads only
     `close_requested`), SSH 32 s quiet loop. SSH lifecycles 20 → 2; the
     failures test keeps only the denied key.
   - `server_mobile_changes_…` lost its config branches (now
     `server_mobile_changes_preserve_local_connections`).
   - `cargo nextest run -p loopflow --lib -E 'test(transport::)'`: 104/104,
     72 s wall under that load. Clippy `--lib --tests --test transport_tests`
     clean. SSH cases not run here (Linux CI).
   - Compressed: owner cases are named `owner-…` instead of a second name
     list; five held-terminal `SessionStart` requests became
     `ServerProof::start_held`. `relay_tests::` 11/11, 2.5 s wall;
     `transport_tests -E 'test(held)'` 7/7; clippy clean.
5. **Done (2026-11-13).** Steer (Task comment, 2026-11-13): coverage removed
   in #1922 (open, not on main) "to rebuild in this suite with a controlled
   timeout". Slice 3 (`da53314382`) had deleted both legs of
   `continue_session_after_upgrade` with nothing replacing them.
   - Give-up timeout: the frozen resume's only bound is the relay's
     acknowledgement deadline, `Pace.attach` (5 s; `transport.rs` `view`).
     `attaches` returns true on the owner's `LiveTurn` retention before any
     socket read, so nothing else waits on the stopped owner. A probe of the
     deleted frozen leg (Mac, debug, not kept) exited 1 after 6.7 s:
     "Execution owner did not acknowledge attachment…". On #1922's Linux CI
     (run 43518654010) the same leg was still running at the fixture's 20 s
     `wait_exit`; the cause of the Linux overrun is not reproduced here. Both
     the relay's connect to the owner (`UnixStream::connect`, no timeout) and
     the setup before it can block without a bound. The new tests do not use
     that path.
   - Frozen leg → `ops::human_session::tests::a_superseded_owner_keeps_a_live_turn_and_gives_way_once_it_settles`
     (0.04–0.12 s). The owner is a `/bin/sleep` child (another executable than
     `LF_BIN`), it holds a live turn and its socket never answers: `attaches`
     is true, the owner is not signalled and `session_attachment` is unchanged.
     Capture, provider thread and Task binding are not read separately: only
     a takeover writes them, and none runs (read from the code, not asserted).
     The relay half (no acknowledgement → exit with the cause, typed input not
     delivered) is the existing `owner-unacknowledged` relay case. A mutation
     dropping the `LiveTurn` early return fails the test.
   - Live leg: the same test's second half (turn settled → `attaches` ends
     the superseded owner and returns false). The kept two-build
     `resume_takes_over_an_owner_from_another_build` now also waits for a new
     `agent_process_id` beside its `--resume <same thread>` and new-owner
     checks (6.2 s locally). Input order into an attached owner is the host
     bench's input rules; viewer return to a held terminal is
     `a_held_conversation_outlives_its_view`.
   - Not rebuilt: the queued draft in the provider's own terminal buffer
     (provider behaviour, not lf's), and a server restart in the middle of
     the leg (`held_session_survives_server_restart`).
   - Compressed: the test restores `LF_BIN` through `test_ambient::EnvGuard`
     (a failed assertion no longer leaks it). Focused test 0.19 s; clippy
     `--lib --tests` clean.
6. **Done (2026-11-13).** Relay cases: `TEST_PACE` unchanged. Times under load
   average ~570 (Mac, debug): handoff 0.06 s, stalled 0.33 s, closure 0.47 s,
   unacknowledged owner 0.56 s, acknowledged 1.0 s, setup 1.5 s,
   unacknowledged 1.5 s, outage 1.9 s, move 2.2 s, quiet 3.4 s. Child startup
   (~0.5 s) and the windows each rule must span (quiet: 4 × 300 ms silence
   answered; move: 4 heartbeats) take most of that. A shorter silence makes
   false reconnects likely under CI load. Accepted; re-measure on CI at
   gate. `lifetime.rs` `POLL` is now a `#[cfg(test)]` 100 ms const (2 s in
   builds). It paces only the recorder's retries and shell/client rechecks.
   `viewer_state_retries_…` went from ~6 s to 0.33 s; its deadlines are 5 s
   bounds, not `POLL` multiples. CI sum under 400 s is unmeasured until gate.
7. **Done (2026-11-13): one relay bench.** `viewer_tests.rs` is deleted;
   its owner-socket preview cases are `preview-…` cases in `relay_tests`
   under `LOOPFLOW_RELAY_FIXTURE`, `TEST_PACE` and one PTY driver. The parent
   keeps a `ghostty_vt` display and acts on `type`, `inspect` (screen reading
   once a marker arrives), `interrupt` (SIGTERM) and `flood`. Preview cases
   set `attach` and `silence` to `STEP`: no preview rule spans a window, so
   those only bound a failure. Cases: startup failure, cancel, blank, modes,
   completed, startup input, directory; 0.08–0.43 s each.
   - `cancel` is the SIGINT-before-acknowledgement case slice 4 left out
     (SIGTERM through the `ctrlc` handler, exit 130). Its owner now keeps
     its socket open: the old case returned and closed it, so the relay
     restored the terminal on EOF before the signal landed. A mutation
     running no interrupt hooks fails it; before the fix it passed.
   - `Peer::next_request` reads a refused `set_read_timeout` (macOS EINVAL
     after the viewer shuts its end) as the connection ending.
   - `resume_fixture` cases, deleted:
     - `reveal`: `--reveal-existing` argv is
       `human_session::tests::reveal_requires_history_…`; the open path is the
       same as resume.
     - `provider_failure`: a failed provider under an owner is the launcher
       test's `failed_native_provider_settles_once_…` leg; the terminal
       restored after an owner ends before readiness is `preview-completed`.
     - `resume`: `--resume` on the same thread is
       `resume_takes_over_an_owner_from_another_build`; no replay is
       `human_session::tests::automatic_start_…`. The saved screen carried
       across the relay's exec (`relay_with_preview`, `LF_RELAY_PREVIEW`) and
       replaced by live output had no kept test: it is now the last leg of
       `a_held_conversation_outlives_its_view` (6.8 s for the whole test
       locally). Claude's `--name` title on resume is not asserted anywhere
       (`terminal_title.rs` adds it on every Claude launch).
   - `scripts/testing.md` and the tests README name `relay_tests` where they
     named `viewer_tests`.
   - `cargo nextest run -p loopflow --lib -E 'test(transport::)'`: 100/100,
     2.78 s wall (Mac, debug). Clippy `--all-targets` clean. CI sum and SSH
     cases: gate/CI.

## Outside this Task

The frozen-resume overrun on #1922's Linux CI (still running at 20 s where
the Mac probe exited at 6.7 s) is a product question, not a test one: the
relay's `UnixStream::connect` to the owner and the setup before it have no
bound, and lf at a turn boundary is meant to keep passing. Slice 5 does not
exercise that path. Unreproduced; worth its own Task if it recurs.

## Checks

Left for gate/CI: the `transport_tests` CI sum against 400 s, relay case times
on CI, and the SSH cases (Linux only).

- gate: `cargo nextest run -p loopflow --lib --test transport_tests -E 'test(transport::) | binary(transport_tests) | test(human_session::) | test(server::) | test(task_retirement) | test(retirement_tests::)' --test-threads 6` all pass; report summed seconds per group before/after (local before: run once at slice 1 start).
- `cargo test -p loopflow --test transport_tests ssh_shells:: -- --ignored`: Linux CI only (macOS sshd sandbox).
- CI: compare `transport_tests` summed medians with the 1,646 s baseline.

## Docs (written ahead of slices 1–4)

- `rust/loopflow/tests/README.md`: new "Transport tests" section (three layers,
  time seams, forbidden shortcuts); transport paragraphs and the server command
  list now point at kept or moved tests. Each slice checks the names it ships
  against this text.
- `rust/loopflow/src/transport/README.md`: bare Graphs at a terminal run in the
  foreground; bare launches have no owner. (This is the "README line ~87" fix.)
- `scripts/testing.md`: store-wide changes run `test(transport::)` too.
