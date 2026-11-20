Read when the change reaches Loopflow Desktop: its windows, panes or bundled `lf`.

# Packaged Desktop

A packaged Desktop must use its signed bundled `lf` for the QA server too:
signing the bundle changes the CLI hash. Set `LF_DESKTOP` to the QA bundle ID
when using `lf desktop state`; otherwise it addresses the installed app.
Keep the private Machine and provider homes separate from normal Work. Record
window-access failures separately from successful CLI or protocol checks.
If computer use returns `cgWindowNotFound`, inspect the QA bundle with
`lf desktop state --json`; an empty `windows` list establishes no mounted UI.
Do not count a running app process or its background reads as a window test.
Desktop readiness may register a private launchd server. After quitting the QA
app, boot out only the label in that private Machine's `server.plist`; killing
its PID alone lets launchd restart it. Unregister temporary QA app bundles too.

# Loopflow Desktop

For Desktop in a headless run, follow `swift/README.md`'s Testing section:
`scripts/test_desktop.sh --filter '<affected suites>'` builds the app and CLI,
then runs model and production-view checks with WindowServer access denied.
Unset inherited `LF_*` and close stdin as described in `scripts/testing.md`.
`DesktopHeadlessTests/workStates` checks loading, unavailable, empty and selected
Work and taps the production navigation button. Report these as automated
checks, not a live-window walkthrough. Hosted window checks remain optional;
record what was not seen. With a display, `uv run python scripts/loopflow-dev.py
run-debug` builds and launches the development app; use `lf desktop state --json`
and the commands in `docs/surfaces.md` to inspect and drive it.

Use a private `LF_HOME` and an explicit branch `LF_BIN` for experiments. Clear
inherited `LF_*` first; isolate `HOME` and the repository too. Never attach the
Session running this QA step.

For headless relay checks, `lf resume --prepare --json` creates connection data
without launching a provider; repeating it keeps the same Session. Start the
branch's `lf server run` in the private Machine and parse `lf server status` as
JSON (its output is compact). Wait for `ready` before starting a relay, so its
initial readiness request does not register a background service. Stop and reap
the private server and every relay afterward.

A probe compiling `ServerConnection.swift` against real relay publications can
check mailbox delivery and cancellation without WindowServer. Label this as a
component probe: it does not establish mounted panes, hidden-pane rendering,
focus or copy/Find. Keep display diagnostics optional in
headless runs; record those limits and leave the full Swift suite and Xcode
fallback build to gate. See `swift/README.md` for the supported checks.

# Launching Loopflow Desktop

The snapshot and quit probe below needs a display; use the headless checks above
when WindowServer is unavailable.

- Build the CLI and local server: `cargo build -p loopflow --bin lf`.
- Build the debug app: `swift build --package-path swift -Xswiftc -gnone`
  (a few seconds when the build cache is warm). Binary: `swift/.build/debug/LoopflowMac`.
- `uv run python scripts/check_desktop_launch.py` launches it with
  `-ui-test-mode empty-workspaces`, waits for the snapshot and the quit, and
  prints the sampled stacks if it hangs. That mode renders fixture data and
  reads no registry, so it runs safely beside the person's live Loopflow.app.
- To look at the frame, call `check_launch(executable, dir)` from that script
  with a directory you keep. `snapshot.png` is written there.
- `scripts/release-loopflow.py` parses no flags. Even `--help` starts a full
  release build.

For a live window on private data, build a world and launch the Dev app on it:
`target/debug/lf-world build one-task /tmp/lf-qa-<name>/world`, then
`uv run python scripts/loopflow-dev.py run-debug --world <world>`. It installs
`~/Applications/Loopflow Dev.app` and stays in the foreground until the app quits.
Run the app's bundled `lf` with the world's `env.json` variables and
`LF_DESKTOP=com.loopflow.mac.dev`; the control socket is under the world's
`LF_HOME`, so `lf desktop state --json` reads only that app.

- Check the screen first: `ioreg -n Root -d1 -a | grep -c CGSSessionScreenIsLocked`
  prints 1 when it is locked, and then `screencapture` returns a black frame
  ("could not create image from window" for `-l <id>`). The app still launches
  and answers its socket, so say which checks were socket reads and which were seen.
- The app registers `com.loopflow.server.<hash>` for the world's home with the
  real `launchctl` (the world's refusing one is not used). After quitting it,
  stop the world as `.lf/qa/auto.md` describes.
- The Auto toggle, Claim, Unassign and Abandon have no `lf desktop` verb:
  `lf desktop state --json` shows the Task page's `run_control`, and the views
  are covered by `DesktopHeadlessTests`.
- Drive it over the socket: `lf desktop ls`, `lf desktop goto 1`,
  `lf desktop run --task <id> '<line>'`. Do not use `lf desktop view` or
  `lf open` there: they open a `loopflow://` URL, and when
  `/Applications/Loopflow.app` carries the same bundle ID macOS starts that app
  on the world instead. It then owns the socket, and its panes run the installed
  `lf`.
- `lf desktop enter <tile>` reaches only the shown Working tiles; Backlog Tasks
  have no socket route, so their pages need a person or headless view tests.
  `lf desktop move -p <task> --destination <task>` writes the shared Task order;
  `lf desktop do working-tasks` and `do back` navigate.
- Look at the window with `screencapture -x -o -l <id>`; list window IDs with a
  `swift` script over `CGWindowListCopyWindowInfo` (owner `Loopflow Dev`).
- `lf desktop send` refuses control characters. To make a held conversation
  report activity, write `ESC]7501;state=working:app=claude-code ESC\` (or
  `state=idle`) to its stand-in's tty (`ps -o tty` of the `bin/claude` child of
  the world's holder); `lf task list --json` then shows `busy`/`available`.
- Wait for the app with `pgrep -x "Loopflow Dev"`: `pgrep -f` with the bundle
  path matches the waiting shell's own command line. The first `run-debug`
  builds the bundled `lf` in its own target directory (28 minutes on a loaded Machine).
- An install that fails with "Third-party inputs changed" means `Cargo.lock`
  changed: `uv run python scripts/third_party.py refresh`, and commit the result.
- Desktop puts `~/.local/bin` ahead of the world's `bin` on its panes' `PATH`,
  so a provider Desktop starts is the real `claude` (check `lsof -p <pid>` txt).
  Start the Task's conversation first from a PTY running the prepared
  `open_argv` under the world's `PATH`; Desktop's panes then attach to that
  stand-in. `lf desktop do refresh-session -p session`, `desktop send`/`send-key`
  and `desktop capture --region scrollback` drive and read the pane; a pane's
  `attachment` reads `opening`, `usable` or `failed`. Reuse no markers across runs: scrollback keeps earlier replies.
- Reinstalling the bundle changes its `lf`: the running private server then
  answers "uses another executable" and panes lose their surfaces. Restart that
  server with the new bundled `lf`; held providers survive in the old holder.
- To see what a pane inherited, compare `lsof -p <app pid>` pipe addresses with
  those of its children and of the PIDs in the world's `processes` table.
- Quit with `NSRunningApplication(processIdentifier:).terminate()` from a
  `swift` script; both apps answer to one bundle ID. Then end the world's owners
  by PID (check `LF_HOME` in `ps eww`) and boot out the label in its
  `server.plist`.
- `scripts/loopflow-dev.py install` builds and installs without launching; then
  `open -n --env …` the bundle yourself to control launch timing. While the
  screen is locked (`CGSSessionScreenIsLocked`), `screencapture` is black and
  `-l <window>` fails: sample `lf desktop ls --json` instead. Its `reading`/
  `reason` are the window's work status and `list` the sidebar rows.
- To run beside another QA app, copy the installed bundle, set its own
  `CFBundleIdentifier` with `plutil -replace`, `codesign -f --deep -s -`, and
  `open -n --env …` it with `LF_DESKTOP` set to that ID. Conversations the app
  opens run the account's real `claude` (with the world's `CLAUDE_CONFIG_DIR`, so
  at sign-in) even when you start the copy's bundled `lf server run` first; a CLI
  `lf checkout` keeps the world's stand-in. OSC 7501 reports written to the
  conversation's tty still drive activity; avoid Graph runs there. Stop the
  holder's children by the holder's PID, then unregister the copy with `lsregister -u`.
- On the Working Tasks Dashboard (`lf desktop working-tasks`),
  `lf desktop move -p <task> --destination <task>` moves a tile and writes the
  shared `task_orders`; Wave tiles are not enterable there, so the Wave backlog has
  no socket route. A locked screen also denies accessibility reads
  (`AXIsProcessTrusted` false), so header text is left to headless view tests.
- `open` hands the app the caller's environment. From an lf-launched shell,
  launch with `env -i HOME=$HOME USER=$USER PATH=/usr/bin:/bin open -n --env …`,
  or the app carries this Session's `LF_AS` and `LF_CAPTURE_KEY`.
- With the screen locked, accessibility reaches the menu bar but no window
  content, and events posted with `CGEvent.postToPid` never arrive: nothing in
  a window can be clicked or typed. Pictures still work: launch with
  `LOOPFLOW_UI_TEST_MODE=live`, `LOOPFLOW_UI_TEST_SNAPSHOT_PATH=<png>` and
  `LOOPFLOW_UI_TEST_DELAY=<seconds>`; the app reads the real server, renders
  itself to the PNG and quits. `LOOPFLOW_UI_TEST_SELECT_BRANCH=<wave name>`
  shows that Wave, and `LOOPFLOW_UI_TEST_WIDTH`/`_HEIGHT` size the window (820
  wide leaves the backlog about 310 points). That mode serves no control socket.
- `LOOPFLOW_UI_TEST_SELECT_BRANCH` alone, without the test mode, opens a normal
  app on that Wave, so `lf desktop capture -p backlog` reads its backlog over
  the socket. Change Work with the CLI or the server socket and capture again:
  the list follows within a second.
- To delay or break Work reads, SIGSTOP the private server. Desktop registers
  it under launchd, so `kill -9` restarts it within seconds; for a sustained
  outage boot out the `server.plist` label, then `lf server run` to recover.
  The world's holder outlives both; stop it and its stand-ins by PID
  (`ps eww` shows `LF_HOME=/private/tmp/…`).
