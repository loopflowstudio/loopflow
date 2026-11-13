# LOO-887: rebuild the transport tests around the current design

Stacked on `jack-heart/way-faster-gate` (PR #1920, open): build on its seeded
fixture homes, its host-level grace test and its three-cycle shell test. Its
notes and CI timings were cleared from scratch at publish; read them with
`git show c4e1251328:scratch/test-speed.md` and `…:scratch/test-timings.tsv`.

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
  5 s attach, a full 12.7 s reconnect cycle) except the one named below.
- Every rule a deleted real-process test asserted is asserted by a named
  in-process test (table below) or deliberately dropped with a reason.
- New host-rule tests each run under 100 ms; relay-rule cases under 1 s.
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
  `viewer_tests.rs` already runs the relay by re-executing the lib test binary on
  a PTY against an in-test fake owner socket.
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
- Fixed sleeps in tests: 3 s (line ~1387), 6 s (~1855), 5 s (~2322), 3 s (~2864),
  8 s (~4007), 4 s (~4743), 4 s ×2 (~5624), 6 s shell sleep (~6010), 32 s
  (~7690), 32 s quiet loop in `ssh_shells.rs:704`. All go.
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

### 2. Relay rules: re-exec'd lib test binary on a PTY (`viewer_tests.rs` pattern)

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

### 3. Real-process surface tests (`tests/transport_tests.rs`, target ≤ 35)

Kept because only real processes, PTYs, sockets or exec prove them (one per
surface, merged where they share a fixture):

| Surface | Kept test (merged from) |
|---|---|
| Owner wiring | `every_viewer_starts_with_the_snapshot_and_shares_live_output` (protocol smoke), `detached_owner_keeps_no_descriptor_its_terminal_inherited`, `owned_program_does_not_inherit_the_desktop_pane_relay` (+ held counterpart assertion), `pipe_eof_delivers_all_input_before_the_command_exits`, `viewer_stream_over_pipes_preserves_terminal_input_size_and_exit` |
| Bare launch | `bare_lf_runs_its_provider_in_the_foreground_and_ends_with_its_terminal`; placement: `first_native_launch_keeps_only_valid_desktop_terminal_placement` + `held_desktop_launch_links_its_session_to_the_pane` → one |
| Launcher → provider exec | `a_terminal_launcher_becomes_its_provider_and_the_owner_records_its_exit` (+ `failed_native_provider_settles_once_with_its_own_code`, parent==owner check from `opening_a_prepared_conversation_runs_one_owner_and_one_relay`, one Graph case from `native_exit_preserves_returning_launcher_results`), `native_pending_request_survives_creator_exit_without_settling_capture_or_replay`, `codex_original_request_channel_and_turn_survive_all_viewers`, `interactive_graph_settles_each_step_before_starting_the_next` (under a test PTY, no owner) |
| Headless | `headless_graph_settles_its_original_step_after_creator_teardown` (+ `a_headless_run_survives_without_viewers`; no 4 s sleep) |
| CLI open/JSON | `conversation_open_here_confirms_attachment_and_does_not_take_existing_input`, `prepared_start_preserves_a_claim_time_winner`, `resume_json_returns_after_relay_and_viewer_takeover`, one launch-once JSON test (`resume_fork_json…` + `session_new_json…` + `resume_json_returns_once_when_it_starts_the_owner`) |
| Build skew / takeover | `resume_takes_over_an_owner_from_another_build`, `resume_ends_an_unanswering_superseded_owner_of_a_completed_session` (+ answering case, + takeover leg of `opening_takes_over_from_an_owner_that_never_answers`), `a_held_view_meeting_another_build_reopens_as_the_installed_lf` |
| Local relay reconnect | `local_terminal_reconnects_without_replaying_input_or_taking_newer_claims` (one owner for its three cases) |
| Server relay | `server_preserves_owner_and_graph_across_restart` (+ `server_move_here_preserves_draft_provider_and_graph`), `server_reconnect_preserves_newer_viewers_input_and_geometry` (terminal=false only), `a_stalled_bridge_leaves_the_healthy_bridge_and_control_checkpoint_live`, `server_startup_cancels_its_pending_installer_when_pane_closes`, `server_status_does_not_install_or_create_work` |
| Held terminals | `a_held_pane_reads_reconnecting_until_the_next_server_answers` (+ `server_initial_connection_reports_opening_until_input_is_acknowledged`), `held_session_survives_server_restart` (trimmed to what `registry_tests` lacks), `a_held_view_waits_for_a_holder_stopped_at_server_startup` (+ `session_attach_waits_…`), `a_launch_refused_for_a_stopped_server_leaves_nothing_to_resume`, `held_terminals_list_one_holder_process`, `server_threads_do_not_grow_with_held_terminals`, `task_cleanup_ends_an_abandoned_tasks_held_terminal`, `a_held_conversation_outlives_its_view` (no 5 s sleep; it is the coverage left after the parent deleted the detached-limit test) |
| Desktop shell | `desktop_shell_reconnects_without_replay_and_close_settles_its_jobs` (one run, no 4 s sleep, absorbs the `getsid` sweep of `desktop_shell_repeated_lifecycles_…`), `completed_task_cleanup_signals_shell_owner_and_settles_jobs` |
| SSH (Linux CI) | `ssh_shell_lifecycles_…` at 2 cycles; `ssh_prepared_viewer_…` without the 32 s quiet loop; `ssh_shell_failures_…` keeps the denied-key case, destination-side cases move to Desktop shell tests |
| Ignored acceptance | Rex ×3, mobile native ×3, launchd registration ×2: stay `#[ignore]`, untouched |

### Moves out of the transport suite (not transport rules)

| Rule | Destination |
|---|---|
| superseded-owner retention (5 `unanswering_superseded_owners_preserve_*`, `unanswering_superseded_owner_preserves_pending_turn_and_queued_input`) | store `owner_retention` table test (`store/sqlite/sessions.rs:714`) + one `human_session::attaches` case |
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

Duplicates or guards for removed mechanisms; each one's rule is covered above:
`automatic_command_attachment_preserves_an_existing_viewers_control`,
`prepared_reveal_preserves_a_later_controlling_viewer`,
`prepared_start_preserves_a_later_controlling_viewer`,
`a_delayed_owner_keeps_exclusive_ownership_of_the_provider_wait` (guards a removed timeout),
`native_program_status_detects_support_before_a_viewer_attaches` (`output.rs:405`),
`viewer_stream_waits_for_destination_resolution_before_attachment` (asserts only that 60 s > 6 s),
`completed_task_without_checkout_ends_a_conversation_with_typed_input` (screen-save assertion joins the shell cleanup test),
`returning_task_viewer_resets_cleanup_without_another_sweep` (= `task_retirement.rs::returning_viewers_reset_retirement_and_retry_without_a_sweep`),
`sessions_keep_serving_when_their_lf_executable_goes` (no executable watcher exists; resume tail joins the takeover test),
`unanswering_superseded_owner_preserves_pending_turn_and_queued_input`,
`resumed_session_completes_turns_after_build_replacement_and_viewer_return` (owner-hosted Task conversation predates #1887; restart leg = `registry_tests`),
`server_reconnect_refreshes_expired_output_without_replaying_input` (= spool host rule),
`server_failed_setup_retry_only_reconnects_and_preserves_newer_claims`, `server_reconnect_retries_after_exhaustion_…`, `server_reconnect_reports_unavailable_…` (→ one relay exhaustion rule),
`server_reconnects_a_writable_bridge_that_stops_answering`, `server_heartbeats_keep_a_quiet_provider_attached`, `viewer_stream_heartbeat_expires_only_the_silent_viewer_while_output_continues` (→ host + relay rules),
`server_move_here_lost_ack_…`, `server_move_here_pending_action_is_cancelled_by_pane_closure`, `server_reconnect_cancels_when_native_pane_closes`, `status_channel_failure_keeps_relay_alive_…` (→ relay rules),
`desktop_shell_repeated_lifecycles_leave_no_owned_jobs_or_sockets` (merged),
and every real-process test whose rule moved to layer 1 (the viewer-protocol cluster: `first_snapshot_uses_the_new_viewers_dimensions` through `viewer_input_survives_its_disconnect`, the control-claim cluster, `session_viewers_retain_until_the_last_leaves_…`, `slow_observer_…`, `a_locked_store_…`, `only_controlling_viewers_…`, `viewers_enter_the_execution_directory_…`, `zero_viewers_preserve_shell_jobs_…`, `viewer_stream_backpressure_…`).
Unused fixture helpers go with them. `lifetime.rs` tests (14 s on `POLL` sleeps) get the same `#[cfg(test)]` timing value if it is one line; otherwise leave them.

## Forbidden

No env var or CLI flag that shortens a production timeout; no factory trait or
clock trait; no second protocol host for tests (the bench is `Hosted` itself);
no test that waits out a production timeout except a named fallback above.

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
   - `viewers_enter_the_execution_directory_and_later_ones_the_selected_directory`
     stays real-process: the bench covers reported directories, but the
     selected-directory half needs an `ExitRecord` whose Session is in the store.
     Port it with a stored Session or keep it as the Owner wiring test.
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
   `ATTACH_TIMEOUT` are gone, host-side `HEARTBEAT_TIMEOUT` stays. Seven cases,
   0.5–3.1 s wall each on a loaded Mac (child startup ≈ 0.5 s; exhaustion and
   quiet-window cases ≈ 2 s because `Status::wait` polls in 50 ms steps). A
   mutation disabling the relay's silence check fails the quiet/silent case.
   Deleted 10 real-process tests, 218.5 s of CI medians; `transport_tests`
   holds 95. Gaps left on purpose:
   - Relay cases use only `Host::Terminal`; owner bridging through the server
     ticket (`bridge_existing`) stays covered by the kept
     `server_reconnect_preserves_newer_viewers_input_and_geometry` and
     `server_preserves_owner_and_graph_across_restart`.
   - Pane closure while a Move here is pending (relay SIGSTOPped) is not
     ported; the closure case covers a pending reconnect and `take_move`
     checks `closed()` before consuming.
   - Local (non-server) relay rules from the design — control handoff notice,
     a host that stops reading input — are not in `relay_tests` yet; their
     real-process tests are in the kept local-reconnect set (slice 4 decides).
3. Non-transport moves (store retention, human_session, server, retirement).
4. Merge the kept real-process tests; remove fixed sleeps; SSH trims. (README
   bare-Graph fix and `scripts/testing.md` routing already landed with the docs.)

## Checks

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
