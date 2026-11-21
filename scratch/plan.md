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

## Remaining

### Desktop pane launch

Have `SessionItem.launchArguments` in `swift/LoopflowMac/Views/SessionsView.swift`
pass `--unfocused` for selected and warm panes. For prepared SSH commands,
place it before the local `--` separator. Keep a directly opened native
`OwnerStream` in `swift/Loopflow/Services/OwnerProtocol.swift` explicitly
focused. Check the argv and wire assertions in `WorkCacheTests`,
`WorkDestinationTests`, `SessionsStoreTests` and `OwnerStreamTests`.

Pinned Ghostty `a60e9e2` sends its current focus when mode 1004 is enabled
(`src/termio/stream_handler.zig:766`). Its initial terminal focus is true
(`src/terminal/Terminal.zig:132`). Desktop's `GhosttyTerminalView.syncSurfaceFocus`
sets focus from the key window and first responder when creating a surface.
The source inspection establishes the mechanism. The mounted behavior is a
QA check.

### Dev opens only visited Work

PR #2007 is now merged into this checkout. `scripts/loopflow-dev.py run`
launches on the live Machine directly. The old description-based Machine
copy path is gone. There is no launcher change in this Task.

Set the existing workspace registry to open unvisited Work only in release
Desktop. In Dev, use the same deliberate visit to decide which Work may
prepare a conversation and whether its placeholder is waiting for an opening.
`openSelectedWorkspace(deliberate: false)` handles restored selection.
Prefetch can still build workspaces and read saved layouts. Release Desktop
keeps its saved warm tabs and worktree retention.

- Follow both `SessionsWorkspaceRegistry.openWarmConversations` and
  `WorkModel.openWarmConversations`. The latter presents known Sessions
  before the registry starts preparation. `conversationIsOpening` also
  predicts an opening and can hide the open action. A guard in the
  preparation loop alone leaves an unvisited pane displaying a spinner.
  Use the same decision for the placeholder and background preparation.
- Record deliberate visits for the life of the registry. `recentTasks`
  already includes prefetched Tasks and Tasks selected by
  `focus(deliberate: false)`, so it cannot answer whether someone visited.
- Exercise a restored board and saved pane layout. `presentView` can select
  a tile without a deliberate visit, and `WorkspacePanesView` can mount a
  retained shell from saved intent without going through Session
  preparation. Check that either startup path starts no terminal before
  the first deliberate visit.
- Add a focused Swift regression covering prefetched Tasks and primaries,
  restored selection, and the first deliberate open. Check that the release
  registry still warms the same Work. Run
  `scripts/test_desktop.sh --filter 'WorkspaceRetentionTests|WorkspacePanesTests|SessionsStoreTests|WorkCacheTests|WorkDestinationTests|OwnerStreamTests'`.
  No Swift changes or checks for this Task's behavior have run yet.

The user-facing text in `docs/sessions.md`,
`rust/loopflow-execution/src/transport/README.md`, `docs/dev/server-first.md`
and `swift/README.md` was drafted before the implementation. Confirm the
flag and Dev behavior it describes when those paths land.

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

Next wire Desktop's pane launch and Dev's deliberate-open rule together, then
verify that the first deliberate visit opens one pane with the expected argv.
