# 2ac1ee — A Session follows the view someone is at

Opening a background view leaves a running Session's PTY size, colors and
focus reports alone. A person taking focus or typing moves ownership to that
view. When it leaves, ownership falls back to the most recently used live
view. A normal terminal `lf resume` still takes the size on attach. A relay
reattaches with the last focus it observed. Dev on the live Machine opens
terminals only for Work someone deliberately visits.

The Task brief says “Consider an optional passive / second-class viewer flag”.
The design uses a single focus rule for all viewers. The choices made for
review and the version-skew limits are in [questions.md](questions.md).

## Next: Desktop says "background" in the pane's environment

Desktop builds from this change run every Session pane as
`lf session resume <id> --unfocused`. Loopflow Dev runs the installed `lf`
(`swift/README.md`), and the installed `lf` 0.16.0 answers
`error: unexpected argument '--unfocused' found`, so every pane Dev opens
fails until `lf install`. QA missed it by running Dev against the branch `lf`.

Desktop stops editing the server's `open_argv`. It sets `LF_RELAY_FOCUSED=0`
in the pane's environment, beside `LF_SERVER_STATUS`, which already marks a
Desktop pane's relay. An older `lf` ignores the variable and opens the pane
as it always has; Dev's wait-for-visit still keeps it off unvisited Sessions.

- Swift: `SessionItem.launchArguments` goes back to `record.openArgv` plus
  `--agent`. The pane's surface `env` carries `LF_RELAY_FOCUSED=0` for
  Session panes, local and SSH (the SSH pane's local `lf session attach --`
  relay inherits it). Test expectations lose `--unfocused` and one test
  asserts the pane's environment.
- Rust: `transport::initial_focus` is the one reader: focused unless
  `--unfocused`, `LF_RELAY_FOCUSED=0`, or stdin is not a terminal. `relay()`
  uses it. `start_held` drops the variable from the holder's environment, so
  an agent's own `lf` calls never inherit it.
- `--unfocused` stays on `lf resume`, `lf session new` and
  `lf session attach --` for a person. Remove what only Desktop used if
  nothing else calls it.
- Docs: `transport/README.md`, `swift/README.md`, `docs/sessions.md` and
  `.lf/qa/desktop.md` say Desktop panes start in the background, without
  naming the flag.

Checks: a PTY `lf session resume` with `LF_RELAY_FOCUSED=0` sends
`focused:false` on its first attach and leaves a focused view's size alone;
the held agent's environment has no `LF_RELAY_FOCUSED`; the Swift launch
tests pass with plain argv.

## Built

Commit `0c7f81d2ba` implemented the holder side. `Request::Attach.focused`
is optional on the wire. Omission preserves older relays' behavior.
`focused: false` leaves the live sizing owner in place, including when two
views have never received focus or input. The reply names the owner at the
time of the attach. An unfocused view still paints at the owner's size.

The plan had ownership settling after frames had been processed. In
`transport.rs` and `transport/hosted.rs`, focus-in and input now update the
owner before the next frame. That matters when the next frame is a focus-out
or a terminal query answer. When the agent requests focus reports, only the
current owner can send focus-out. Each viewer already carries its recency and
name, so `Sizing` keeps only the owner's ID and the recency counter. On
departure it chooses the most recently used survivor.

Holder checks from the first slice:

| Command | Result | Runs again |
| --- | --- | --- |
| `cargo test -p loopflow --test transport_tests an_unfocused_view_leaves_the_session_with_the_view_in_use` | Passed. Three views on one held PTY covered the attach reply, size, absence of a WINCH marker, focus filtering and fallback on departure. | CI |
| `cargo test -p loopflow-execution --lib equally_inactive_views_keep_the_live_owner` | Passed. A second unused view leaves the first in charge. | CI |

Commit `87437bbe01` carried focus through first attach and local reconnect.
Both local and SSH relays start focused for a TTY unless given `--unfocused`.
Piped relay stdin starts unfocused. The relay observes keys and focus reports
in arrival order, including the rest of a read after a write fails. A focus-out
following typing in that read needs to win over the typing when reattaching.
A local reconnect sends the last observed focus on both continuation and
repaint. The restart test covers the local relay.

In `rust/loopflow/src/lf/commands/ssh.rs`, prepared SSH commands put
`--unfocused` on the local `lf session attach` before `-- ssh …`. Direct and
JSON prepare-and-view SSH routes pass the same choice to the local relay.

| Relay check | Result | Runs again |
| --- | --- | --- |
| `cargo test -p loopflow --test transport_tests a_resumed_relay_keeps_focus_across_server_restarts` | Passed on a private Machine. Ordinary TTY resume took 80 columns. Piped and unfocused resume kept 120. After two server restarts the same relay kept 120 after focus-out and took 80 after typing. | CI |
| `transport::tests::a_held_screen_the_relay_drew_into_is_refreshed_not_continued` | Passed for both focus values on a positioned continuation and a fresh repaint. | CI |
| `machine_commands::remote_session_open_returns_an_owner_pinned_terminal_command` | Passed. Remote prepare placed the flag on the local relay and kept the SSH command intact. | CI |

### Desktop panes and Dev visits (`81c612cbd2`)

Desktop Session panes add `--unfocused` to the viewer command, before the
local relay's `--` for prepared SSH commands; the section above replaces this
with the pane's environment. A directly opened native `OwnerStream` sends
`focused: true`.

`recentTasks` includes restored and prefetched Work. Dev instead uses the
workspace's existing `lastVisit` to admit it to terminal preparation and
mounting (`SessionsView.swift`, `WorkModel.swift`, `WorkspacePanesView.swift`).
Restoring a selection or laying out a board does not record a visit. An
unvisited saved shell shows an Open shell action. Release Desktop keeps the
warm set. PR #2007 moved Dev onto the live Machine and its installed CLI.

Pinned Ghostty `a60e9e2` sends its current focus when mode 1004 is enabled
(`src/termio/stream_handler.zig:766`). Desktop's
`GhosttyTerminalView.syncSurfaceFocus` sets focus from the key window and
first responder when creating a surface. Mounted behavior is a QA check.

`swift build --build-tests --package-path swift` passed. Seven focused tests
passed with `scripts/test_desktop.sh --skip-build`: Dev restored a selected
Task without preparing any Session, its first visit prepared just that Task,
and a restored board shell rendered its open action without a Ghostty view.
Release prefetch, sidebar opening, provider switching, SSH argv and native
attach framing also passed. The wrapper's initial Cargo build lacked this
shallow checkout's migration order and failed on `graph_process_task`; the
Swift run used the existing checkout CLI. CI runs the suites.

### Compress checks

- `cargo test -p loopflow --test transport_tests a_resumed_relay_keeps_focus_across_server_restarts` passed. CI repeats it.
- `cargo test -p loopflow-execution --lib creation_retry_reconciles_publication_but_never_repeats_a_stop_or_append` passed. CI repeats it.
- A PTY ran `lf session attach [--unfocused] -- <peer>` against a local protocol reader. The first attach frame carried `focused: true` by default and `false` with the flag. Both relays exited on the peer's close frame.

## QA (`scratch/qa.html`)

Held-terminal sizing, focus filtering, color answers and Dev's
wait-for-visit held on a private Machine. QA found that the server's repaint
nudge after a restart restored the size it took the terminal up at, overriding
a view that took ownership during those 0.4 s. `Nudge` now reads the current
size at both steps (`registry.rs`). The test
`a_view_sizing_the_terminal_during_the_repaint_nudge_keeps_its_size` passed and
fails when the fix is reverted. CI runs it again.

Mounted Ghostty focus still needs an unlocked display. This run's screen was
locked throughout. The release Desktop warm set and SSH relays were not run.
