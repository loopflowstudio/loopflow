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
checks. With a display, `uv run python scripts/loopflow-dev.py
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

# Loopflow Dev on the live Machine

Use an unlocked, idle display. Set up the isolated Dev bundle below before
launch. The normal launcher replaces the shared Loopflow Dev bundle.

1. Record the server instance and executable hash with the installed entry
   (`~/.local/bin/lf server status`) and note the running holders with
   `pgrep -f 'lf holder run'`. Use that same `lf` for the commands below.
2. Make a visible Swift edit, build and open the isolated bundle as described
   below, and check the change over the live backlog.
3. Create a disposable Task with the installed `lf`. Watch it appear in Dev,
   rename it with `lf task update <id> --title 'Renamed QA Task'`, and check
   the row updates without relaunching. Abandon the Task afterwards.
4. Attach to a disposable running Session with `lf resume <session>` and open
   that same Session in Dev. Check that both views show the same output and
   that a line typed in Dev appears in the terminal view. Close and reopen
   the Dev pane and verify the Session is still running.
5. Compare the server instance, executable hash and original holder PIDs
   with step 1.

Dev reads no Work until its window is on screen. With `open -g`, the window
stays off screen on “Opening repository…”, the server logs only `config.theme`
and `lf desktop state` lists no windows. A mounted check takes focus, so wait
until nobody is at the keyboard.

`LF_DESKTOP=com.loopflow.mac.dev lf desktop state --json` inspects Dev's reads
even with the screen locked. Visible rendering and keyboard input need an
unlocked display.

To build the isolated bundle, import `scripts/loopflow-dev.py`, set
`DEV_APP` to `~/Applications/Loopflow Dev QA.app`, `DEV_EXECUTABLE` to
`DEV_APP.stem` and `DEV_BUNDLE_ID` to a unique ID ending in `.dev`. Run
`cmd_build()` and, if it succeeds, `_install_dev_app()`.
LaunchServices won't resolve a bundle ID under `/tmp`, so `lf desktop view`
fails there with `LSCopyApplicationURLsForBundleIdentifier`. Register with
`lsregister -f`, open it with `LOOPFLOW_DEV_WAVE_REPO` and `--repo` after
clearing `LF_*`, and unregister it and delete it afterwards.

An isolated bundle ID ending in `.dev` gets Dev behavior (terminals wait for
a visit). With the screen locked, Dev still mounts its window and reads Work,
so launch-time attaches can be counted: list `pgrep -f 'session attach'`
children of the app's PID. A Dev pane's relay has `LF_RELAY_FOCUSED=0`;
Ghostty never reports focus-in while locked. Point it at a private Machine
with `open -n --env LF_HOME=… --env LF_BIN=… --env PATH=…` (stand-in agents on
that `PATH`); quit with `osascript -e 'quit app id "<bundle id>"'`, which
reopens the last selected Task on the next launch.

These steps work with the screen locked, using that bundle ID in `LF_DESKTOP`:

- `lf desktop view 'task:<id>' --repo <repo>` and `lf desktop enter '<title>'`
  select a Task. Tiles are addressed by title, and `enter nosuchtile` lists
  them, which is how to watch a rename arrive. `enter` needs a window showing
  tiles; from a Task page, `lf desktop back` until it prints `repository`.
- `lf --task <id> desktop focus -p session` is a deliberate visit: on a
  restored selection it starts that one Session's relay.
- `lf --task <id> desktop capture|send -p session` reads and types into the pane.
  Without `--task`, these commands address the cwd's Task. From this
  checkout, that is this Task's own Session pane.
- `lf --task <id> desktop close -p session` leaves the pane `attachment:
  failed`. `show` says it's already shown. `focus -p session` reattaches it.
- For the original view, run `lf resume --task <id> -a claude` in a detached tmux
  (`tmux -L <name>`). First set the Task's notes to say "reply ready and
  wait", so the agent does nothing. `tmux send-keys -l` types from that side.
  Count `claude --name <title>` processes to check that no second provider
  started.

On launch, Dev can restore retained workspaces from the Machine's saved layout
and start `lf session attach` for each. Use a Machine with only disposable
Sessions if this restoration can attach to personal terminals. Quit Dev when
the check is done.

For a change to the branch server, follow the private setup in
[served launches](served-launches.md). For mounted app checks, use the
packaged app with its signed helper or launch a Dev bundle with `LF_HOME` and `LF_BIN`
after clearing inherited `LF_*`. Use the app's bundle ID in `LF_DESKTOP` and
issue its control commands with that same private home. Stop the private
server, holders and providers afterwards as described in [resume](resume.md).
