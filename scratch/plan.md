# 2ac1ee — A Session follows the view someone is at

A background view leaves a running Session's PTY size, colors and focus
reports alone. Focus or typing takes ownership. On departure the most
recently used live view takes over. An ordinary terminal `lf resume` takes
the size on attach, and a reconnecting relay sends its last observed focus.
Dev opens terminals only for Work someone deliberately visits.

The brief says “Consider an optional passive / second-class viewer flag”.
The implementation uses one focus rule for all viewers. The choices are in
[questions.md](questions.md).

## Built and checked

- **Prepared commands.** PR #2007 moved Dev onto the live Machine and its
  installed CLI. The installed 0.16.0 CLI (build 7127b9d0d) rejected the
  added flag with `error: unexpected argument '--unfocused' found`. Desktop
  moved initial focus into the Ghostty pane's `LF_RELAY_FOCUSED=0`. The
  exact argv prepared by the installed CLI opened a provider stand-in on
  a private Machine. Its old holder still gave the visited pane the size.
  [QA transcript, section 6](qa.html)
- **Prepared argv cleanup.** Commit `34ade5b29e` removed the remaining
  focus flag from local and SSH preparation and deleted its special
  `session attach` parser arm. Resume and New still accept `--unfocused`
  for direct viewing, including SSH. The focused Machine-command test
  `remote_session_open_returns_an_owner_pinned_terminal_command` passed
  with identical prepared argv for both focus options. The transport test
  `an_unfocused_view_leaves_the_session_with_the_view_in_use` passed after
  the deletion.
- **Held terminal and reconnect.** Three views on one PTY verified
  ownership, focus filtering, fallback and no WINCH on background attach.
  In `a_resumed_relay_keeps_focus_across_server_restarts`, a normal TTY took
  80 columns. Piped stdin, the flag and the pane environment left the owner
  at 120. After reconnect, focus-out left it at 120 and typing took 80.
  The provider stand-in received no `LF_RELAY_FOCUSED`. The wire field
  remains optional for older clients.
- **Restart repaint.** QA saw the repaint nudge undo a resize that arrived
  during its 0.4 s pause. Reading the current size at each step fixed it
  in `transport/registry.rs`. The repaint regression test passed and
  failed with the fix reverted. The focused reopening, repaint and color
  tests also passed after compression. [QA transcript](qa.html)
- **Dev visits.** Locked-screen QA saw zero relays on launch and restore,
  and one on the first visit, with both the branch CLI and installed
  0.16.0. Ten focused Swift launch tests passed, covering Dev restore,
  first visit, Open shell for unvisited Work, provider switching, and
  local and prepared SSH argv. Swift checks used the existing checkout
  CLI after the Desktop wrapper's Cargo build failed on
  `graph_process_task` in the shallow checkout. [QA transcript](qa.html)
- **Relay compression.** `cargo test -p loopflow-execution --lib transport::tests::`
  passed all 11 cases. The focused `transport_tests` cases
  `a_resumed_relay_keeps_focus_across_server_restarts` and
  `held_session_survives_server_restart` passed, including uninterrupted output
  across restart. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
  passed. CI repeats these checks.

- **Real SSH.** On mini-heart, under a private home, a remote-TTY view
  owned the Session at 80. Background attaches left it alone: a local
  `lf --machine … session resume --unfocused` at 120, and a prepared argv
  run with `LF_RELAY_FOCUSED=0` at 100. Focus or typing took the size,
  focus-out from a view not in use was dropped, and each departure handed
  the size to the most recent live view. [QA transcript, section 7](qa.html)

## Next at QA or gate

On an unlocked display, use a disposable Session on a private Machine
with two views at different widths. Open a second release Desktop
window and verify that warm panes leave the active width, colors and
agent focus alone. Focus a mounted pane and verify it takes ownership
before the first key. Run the mounted environment assertion in
`swift/LoopflowTests/GhosttyTerminalInputTests.swift`. The display
stayed locked during every run so far, including the SSH run.
[Desktop QA setup](../.lf/qa/desktop.md)

CI repeats the headless Rust and Swift checks.
