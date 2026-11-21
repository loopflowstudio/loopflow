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

### Desktop panes and Dev visits

Desktop Session panes add `--unfocused` to the viewer command, before the
local relay's `--` for prepared SSH commands. A directly opened native
`OwnerStream` sends `focused: true`.

Dev uses the workspace's last deliberate visit to admit it to terminal
preparation and mounting. Restoring a selection or selecting the first tile
when laying out a board does not record a visit. A saved shell has an Open
shell action before its Work is visited. Release Desktop continues to open
the warm set. PR #2007 already moved Dev onto the live Machine.

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

## Live acceptance at QA

Use a disposable Session in release Desktop. Open a narrower background
view with `lf session resume <id> --unfocused` and check that the running
Session keeps its size and colors. Focus or type in the new view and check
that it takes the size. Close it and check the original width returns. Check
that a background relay which reconnects leaves the active size alone.
Also change pane focus during reconnect. The local relay drains pending
terminal input again after the attach reply in `transport.rs`. The restart
fixture changes focus before stopping the server and does not cover this
interval. Check that Ghostty's focus report after the reply brings the holder
up to date.

Launch Dev beside release Desktop on a disposable repository with a saved
selection and live Sessions. Before interacting, check for both preparations
and new terminal viewers, including any restored shell. `pgrep -lf 'session resume'`
alone misses relays that have already exec'd to `session attach`. Deliberately
open one Task and check that only its terminal starts and can take the size.
These mounted checks have not run.
