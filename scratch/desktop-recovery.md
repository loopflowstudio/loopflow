# Desktop session recovery

2026-11-08 report, Jack Heart:

> (a) sessions that hit usage limits didnt auto transfer to codex accounts with space, despite recent diff attempting ot help with this
> (b) one session (718)  Is stuck with Resume paused goal? and not accepting input
> (c) some sessions just wont open
> (4) there's no "refresh" button as there should be for sessions to deal with situations like this

Observed here: Desktop's LOO-718 pane shows Codex's paused-goal dialog. Its
provider and transport processes are alive. Sending Escape through Desktop's
pane API returned success but did not dismiss the dialog. No goal was resumed.
MTG-252's screenshot names Claude thread e80bd648-0697-4548-b0a4-4a82710fdae8;
that transcript was absent from both native and stored Claude homes when checked.
Desktop logs repeatedly show `select a repository before reading execution location`.

Further observations on 2026-11-08:
- LOO-718's viewer-only reconnect did not restore input. Escape advanced the
  native Codex tty's byte offset by five bytes without dismissing the dialog.
  Stopping idle Codex PID 21650, reopening the same saved conversation, and
  sending Escape dismissed it. The goal stayed paused. The cause inside that
  Codex client remains unknown; this was recovery, not a cause fix.
- The ended relay showed “Press any key to close the terminal” while Desktop's
  connection-input guard rejected those keys.
- MTG-252 already had Codex Session session_99ffa20715cd497fada9abfce92cbaee.
  Its opening messages included “reusme the claude session that was hapening in
  this workspace” and “we were testing depth 4 attetnion with different critic
  strategies”. Bound it as primary, replacing the missing Claude reference.
  `session resume --prepare` resolves Codex and reports Reconnect; the installed
  Desktop still reports no surface for this Task.
- The recent quota change automates Graph fallback. Native Claude's limit hook
  only switches Claude logins; it has no Codex fallback. Native Codex observes
  usage but does not perform that recovery either.

Work remaining:
- Extend interactive quota recovery to eligible accounts/providers. A pending
  question asks whether switching should continue the interrupted work or wait
  for a new message; neither behavior has been implemented here yet.
- Verify MTG-252 opens in Desktop after the repository-location fix is installed.
- Verify the Refresh button and ended-terminal key handling in a built app.
- Trace the native Codex input hang if it recurs; reattachment did not fix it.

Checks: focused Rust location regression passed; 69 Swift tests across five
suites passed; Swift rebuilt after ended-terminal repair; cargo fmt and
`cargo clippy --all-targets -- -D warnings` passed. Native terminal interaction
still needs the built-app check above.
