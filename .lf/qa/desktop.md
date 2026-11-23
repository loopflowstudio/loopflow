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
`DesktopHeadlessTests/workStates` checks loading, unavailable and selected Work
and taps the production navigation button; `EmptyStateTests` checks the empty
repository. Report these as automated checks. With a display, `uv run python scripts/loopflow-dev.py
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
Run that script with `uv run python` from the checkout: the module imports
`boto3`, which only the project environment has.
Load it with `importlib` after `sys.path.insert(0, "scripts")`: it imports
`app_bundle` from beside itself. The Dev bundle carries no `lf` of its own,
so pass `LF_BIN` with `open --env`.
LaunchServices won't resolve a bundle ID under `/tmp`, so `lf desktop view`
fails there with `LSCopyApplicationURLsForBundleIdentifier`. Register with
`lsregister -f`, open it with `LOOPFLOW_DEV_WAVE_REPO` and `--repo` after
clearing `LF_*`, and unregister it and delete it afterwards.

An isolated bundle ID ending in `.dev` gets Dev behavior (terminals wait for
a visit). With the screen locked, Dev still mounts its window and reads Work,
so launch-time attaches can be counted: list `pgrep -f 'session attach'`
children of the app's PID. Session panes start in the background. Check that a
visited Session opens with the installed CLI as well as the branch CLI.
Ghostty never reports focus-in while locked. To check a pane opened by the installed CLI,
make a fresh private Machine whose `bin/lf` is a copy of
`readlink -f ~/.local/bin/lf` and start its `lf server run` from that copy; a
branch-built home may be past the installed CLI's schema. Point it at a private Machine
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

On launch, Dev can restore a selected workspace from the Machine's saved
layout. Verify that restoration starts zero `lf session attach` relays and
the first deliberate visit opens only that Session. Use a private Machine
with disposable Sessions and quit Dev when the check is done.

For a change to the branch server, follow the private setup in
[served launches](served-launches.md). For mounted app checks, use the
packaged app with its signed helper or launch a Dev bundle with `LF_HOME` and `LF_BIN`
after clearing inherited `LF_*`. Use the app's bundle ID in `LF_DESKTOP` and
issue its control commands with that same private home. Stop the private
server, holders and providers afterwards as described in [resume](resume.md).

# Looking at the Task pane with the screen locked

`-ui-test-mode live` renders through the real registry and, with
`LOOPFLOW_UI_TEST_SNAPSHOT_PATH`, writes the window with `cacheDisplay`, which
works while the screen is locked. Pair it with a private Machine (separate
`HOME`, `LF_HOME`, a disposable repository with `planning: {provider: git,
personal: true}`, `lf set --global default_agent claude`, a stand-in `claude`
on `PATH`) running the branch `lf server run`. A repository Graph whose
`cmd: help graph` step an `LF_BIN` wrapper turns into
`while [ ! -f <dir>/go ]; do sleep 1; done; exit 0` holds the Task "running"
at that step; `touch <dir>/go` moves it to the next interactive step
("Waiting for you at …"). Killing a held step fails it, and `lf task run`
starts it again. Put the held step in a sub-Graph (`- graph: make`, with
`.lf/graphs/make.yaml`) so the bar's top-level step differs from the running
concrete step. Run the wrapper and `lf` through a script that does
`exec env -i HOME=… LF_HOME=… LF_BIN=… PATH=… "$@"`: zsh does not split a
`$E` variable into words.

- Launch the isolated bundle with `env -i HOME=$HOME PATH=/usr/bin:/bin open -n
  --env LF_HOME=… --env LF_BIN=… --env LOOPFLOW_UI_TEST_MODE=live --env
  LOOPFLOW_UI_TEST_SNAPSHOT_PATH=… --env LOOPFLOW_UI_TEST_DELAY=14 --env
  LOOPFLOW_UI_TEST_WIDTH=… "<bundle>" --args --repo <repo>`. Width 760 is
  where the workflow bar keeps only its sentence.
- Select the Task with `open -g -b <bundle id> 'loopflow://<repo name>/task/<id>'`
  (from `lf open --task <id> --link`). `lf desktop view 'task:<id>'` opens a
  Dashboard named for the expression instead.
- Test modes serve no Desktop control socket (`LoopflowApp.init` starts
  `DesktopServer` only when `AppTestMode.current()` is nil). To switch the
  Task pane's tab, move that `DesktopServer.start()` line out of the `if`
  for the local build only, then `lf --task <id> desktop do details
  page=brief|diff` (no `--task` fails with "No Task selected"). The Task
  page ("Task & workflow") opens only from the pane's menu; for a capture,
  point the `graph-log` action in `DesktopActions.swift` at `show(.task, …)`
  and run `desktop do graph-log`. Revert both before committing.
- Theme: write `theme: "Loopflow Light"` (or a file name in
  `$LF_HOME/themes/`, the imported-theme case) to `$LF_HOME/config.yaml`
  before launch.
- Not reachable this way: Ghostty panes (they open on a visit, and Ghostty
  sees no focus-in while locked, so `desktop capture` says "no native
  surface") and menus. Terminal faces need an unlocked display.
- Skew: restart the private server from a copy of the installed `lf` on the
  same `LF_HOME` (no migrations between them) to see the branch app read
  older responses.

# Driving the app with the screen locked

A production-mode launch (no `-ui-test-mode`) on a private Machine renders the
real windows, and a file-driven hook inside the app can snapshot and click
them. Nothing here needs WindowServer focus.

- Copy `.lf/qa/QAHook.swift` into `swift/LoopflowMac/` and call
  `QAHook.start()` in `LoopflowApp.init` after the appearance store is set.
  Delete both before committing. Build the isolated bundle as above, with
  `sys.path.insert(0, "scripts")` before importing `loopflow-dev.py`.
- Open with `--env LOOPFLOW_QA_HOOK_DIR=<dir>` beside the private `HOME`,
  `LF_HOME`, `LF_BIN` and `PATH`. Write one line to `<dir>/<name>.cmd`; the
  answer appears in `<name>.cmd.out`.
- Give `snap` and `frame` an absolute prefix: a relative one is resolved
  against the app's cwd (`/`), and the `.out` lists a file that was never written.
- Skew without relaunching: start the private server through a symlink
  `bin/lf` and swap the binary it serves (`kill` the server PID, relink,
  start again on the same `LF_HOME`); the running app reconnects to the new
  one, which is how "update the older one" is checked.
- `snap <prefix>` writes every window with `cacheDisplay` (no title bar) and
  lists index, title and size. Indices change as windows open: read the list
  before each click.
- `click <window> <x> <y>` takes points from the top left. Scale from the
  PNG: a 1280 pt window is 2560 px. The hook posts the mouse-up to the queue
  before sending the mouse-down; sent the other way a segmented control spins
  in `trackMouse` and the app hangs. Don't `activate` or `makeKey` in the
  hook: every later click goes dead. The first click after a launch is often
  swallowed; send it twice and look.
- `frame <prefix>` writes each window's frame view, title bar and toolbar
  included; `snap` and a locked-screen `screencapture -l` ("could not create
  image from window") both miss the title bar. The hook's windows are never
  key, so the bar draws lighter than its stated color (`#722f37` reads back
  `#7e4448`); compare bars against each other, not against the file.
- `menu Settings…` performs a main-menu item, which is how Choose Theme
  opens. `close <window>` closes one. Customize answers `click` a few
  seconds after the first; the editor's disclosure groups never open, and
  after the editor opens Apply answers no click either.
- `import <path>` stands in for the Import tile's file panel (unreachable
  locked): it cancels any open panel and imports the file as the panel's OK
  does. `size <window> <w> <h>` shows the "Yours" row. Keep the whole window
  on the display: `tree` prints its frame, and a part below y = 0 answers no
  click. On a 982 pt display 900x1250 sits at y = −384; 1500x900 puts the
  dark section of System mode within reach.
- Imported themes: most Ghostty files (GitHub Light Default, Catppuccin
  Latte) are refused for cursor or selection text. A catalogue file from
  `git show main:…` loads and stands in for a theme saved before a change.
- Packaging refuses ("Third-party inputs changed") after an edit to a
  third-party input (`Cargo.lock`, fonts, Ghostty resources) until `uv run
  python scripts/third_party.py refresh` rewrites
  `ThirdPartyNotices/inventory.json`; commit that. Theme files are not inputs.
- `set -- $pair` does not split words in zsh; copy captures out of the
  private Machine one by one before deleting it.
- Theme: `lf set --global theme "<name>"` on the private Machine, then
  relaunch. An imported theme is a file in `$LF_HOME/themes/`; Ghostty.app's
  `Contents/Resources/ghostty/themes/` has many.
- A Wave's backlog: `open -g -b <bundle id> 'loopflow://<repo>/wave/<wave>'`.
  The Portfolio: `loopflow://portfolio`. It lists Waves only for repositories
  under its folder (`$HOME/src`); a symlink there works, but a Task click then
  opens a second repository window for the symlinked path that stays on
  "Updating…". Sidebar Wave rows did not answer hook clicks.
- `lf desktop <tile-or-dashboard>` with `LF_DESKTOP` set to the bundle ID
  works in this mode (`lf desktop working-tasks`, `lf desktop back`).
- Navigate with `lf desktop goto <n>`, `working-tasks`, `back` and `forward`;
  `lf --task <id> desktop do details page=brief` switches the Task pane. A tile
  shows its workflow sentence only after its Task was visited in that window.
  `lf desktop do new-task` opens the New Task sheet, and `snap` lists a sheet
  as its own untitled window. Card clicks in the chooser preview reliably;
  Apply and Cancel answered one hook click in round 7 (Apply wrote
  `config.yaml`) after missing in earlier runs. If they miss, set the theme with
  `lf set --global theme`. Send Customize once: Done Editing takes its place,
  so a second click closes the editor again.
- Opening a Task changes it: a link to an unstarted Task places its checkout
  (the server then calls it `idle`), and opening a Task whose worktree was
  moved away puts the worktree back. Read `lf plan list --json` for what a
  row is drawn from.
- Relaunching: a window the app restores is titled with the bundle's name and
  stays on "Updating…" while the screen is locked; a link then opens a second
  window on the same repository and the two draw each other's selection. Before
  quitting, close the Portfolio and leave one repository window on a Task that
  is not finished. Stop the app by PID (`osascript` quit is refused while a
  sheet is up), `defaults delete <bundle id>`, relaunch, and use the window
  titled with the repository's name; if none appears in 15 s, open
  `loopflow://<repo>`. Never send a finished Task's link to a relaunched app.
- A theme set from the CLI reaches the running app. Close and reopen Settings
  a few times (`close`, then `menu Settings…`) to check the chooser's grids
  draw: a `LazyVGrid` there drew no cards, or a short Yours row, after a reopen.
- On a display someone is using, `open -n -g` keeps the app behind their
  windows and the hook works as it does locked. `screencapture -l <number>`
  shows what WindowServer composited (to tell an app bug from a `cacheDisplay`
  one), but after Settings is reopened under the same window number it fails
  with "could not create image from window".
- `tree <window>` prints the window's accessibility elements with frames;
  a lazy grid's cards appear there only once drawn.
- Not reachable: hover, drag, the jump menu (its shortcut needs
  a key window, and setting `navigation.palette = .search` draws nothing),
  popovers and sheets opened from a "+" (New Wave never appeared), file panels
  and Ghostty faces. The in-process accessibility tree is empty, so there is no
  finding a control by name.
- A private Machine can be kept for the next run: `rm` its `lf` copy, copy
  the new build in and start `lf server run` again. A Task whose held step
  was running when the old server stopped comes back blocked ("process left
  no exit record"), and opening it does not unblock it: that is the route to
  a blocked Task's page and Brief.
- Window indices move whenever a window opens or the app restores one. Read
  the index by title from `snap` before each `click` or `close`
  (`awk '$2=="Portfolio" {print $1}'`). `lf desktop working-tasks` answers in
  whichever repository window is first; in a restored one it drew "Dashboard
  unavailable". Close Appearance and the Portfolio before quitting.
- Portfolio buttons answer hook clicks: Start on a Roadmap row runs the Graph.
  With the `LF_BIN` wrapper holding `help graph`, the row reads "Moving" until
  `touch <dir>/go`, then "Waiting".
- Stand-in providers exit at once here, so a Session pane shows "Process
  exited" under its opening overlay. That is the stand-in, not the pane.

# Driving sheets and captures with the screen locked

One capture per launch is slow. For a run with many states, add a loop to
`uiTestSnapshot()` in `LoopflowApp.swift` for the local build only and revert
it before committing: with an env variable naming a directory, poll it, and
for each `<name>.req` write `SnapshotService().snapshotWindow(window, …)` to
`<name>.png` and `window.attachedSheet` to `<name>-sheet.png`. Start
`DesktopServer` under the same variable so `lf desktop` works in `live` mode.

- The loop also needs every other visible window written out (`<name>-w<n>.png`):
  the Open Work window is its own window, not a sheet. Send clicks and keys to
  `window.attachedSheet ?? window`.
- Relaunch with `--args -ApplePersistenceIgnoreState YES --repo <path>`. After
  the first quit the bundle restores "no windows" and a later launch sits with
  none (`desktop state` lists `"windows": []`, the log shows only
  `config.theme`).
- With a sheet open, `osascript … quit` fails with "User canceled"; stop that
  one PID. `open -n` beside a live instance leaves two; check `pgrep` first.
- New Task with the QA hook alone (no capture loop): `insert <words>` types
  into the open sheet's field, `key return` sends ⌘↩ (Start), `key escape`
  closes it. `press new-task-file` answers "not found"; click the point
  instead: in the 540 pt sheet File to backlog sits at (74, 308), 33 pt lower
  once a "Not filed." line shows. `lf desktop do new-task` prints the scope it
  opened in (`repository` or `wave:<id>`), which is how to tell a sidebar
  click on a Wave landed. A Wave's own default: `graph: <name>` in
  `.lf/wave/<name>/config.yaml`, committed. Removing that Wave with the sheet
  open should show **Wave unavailable**. Select the repository to enable File
  and Start again.
- The private server's argv is plain `lf server run` (the `env -i` wrapper
  execs it from `PATH`); find its PID with `ps -E -p <pid> -o command=` and its
  `LF_HOME=`. In zsh `kill $PIDS` with a newline-separated list kills nothing;
  pipe the PIDs to `xargs kill`.
- New Task: `lf desktop do new-task` opens the sheet. Type with
  `(sheet.firstResponder as? NSTextView)?.insertText(…)`; ⌘↩ and Esc work as
  `sheet.performKeyEquivalent(with:)` on a synthesized key event.
- A synthesized click presses SwiftUI buttons, sidebar rows and backlog rows.
  Post the mouse-up with `NSApp.postEvent` before sending the mouse-down
  through `window.sendEvent`: a text field or other control that tracks the
  mouse waits for the up event in the queue, and the app hangs without it.
- To type into a SwiftUI `TextField`, find its `NSTextField` by placeholder in
  the content view, `makeFirstResponder` it, then insert into the window's
  first responder (the field editor) and send Return.
- Menus cannot be opened or read: an `NSPopUpButton` behind a SwiftUI `Menu`
  has no items until it tracks. Change the repository's `graph:` to see the
  sheet on another workflow.
- With `LF_DESKTOP` set to the QA bundle ID, `lf desktop view 'task:<id>'
  --repo <repo>` opens its Dashboard in the QA app, and `lf desktop enter
  '<title>'` then selects the Task without opening a conversation. That is
  the way to an unstarted Task's pane: a `loopflow://<repo>/task/<id>` link
  opens its conversation, which claims and places it, and so does
  `lf open --task <id> --link`.
- A fresh `git init` directory with no conversation opens on "No planned Work
  in this repository"; **Open the repository conversation** starts the
  conversation and brings the page, whose backlog reads "Nothing is waiting
  in this repository." beside **New**. Filing a Task brings the page too.
- "Not filed." and "Not started.": stop the private server (`kill` its PID),
  wait five seconds, then `lf desktop do new-task`, type, and press File or ⌘↩.
  After a confirmed refusal, check that the sheet still has the request, Wave
  and workflow. Restart the server and submit from the same sheet. An unwritable `.git` or a repository
  default naming a missing Graph ends in creation recovery (a notice, and the
  Task is filed), not in "Not started.".
- A SwiftUI `ScrollView` that meets the title bar comes back white from
  `cacheDisplay`, background included.
  To see it, build once with the scroll view padded 60 pt from the top; do not
  commit that.
- A stand-in `claude` must answer `--version` (exit 0) and then stay alive
  (`cat > /dev/null; while :; do sleep 5; done`). One that exits fails the
  Graph at its first step with "claude --version failed", and a repository
  conversation reads "Session ended."
- For what a released build does, install a second bundle whose ID does not
  end in `.dev`: it opens a repository's conversation on arrival, so a fresh
  repository shows its page and never the empty state.
- `lf desktop state --json` is one long document; read it through `jq`. A
  pane's `location.worktree` and `attachment` say where it points and whether
  it opened.
- A window's size comes back from the bundle's defaults. Before measuring one,
  quit and `defaults delete <bundle id> "NSWindow Frame open-work"` (or
  `workspace-AppWindow-1`); otherwise a frame from an earlier build is what
  opens.
- "Workspace did not open.": `open -g -b <bundle id>
  'loopflow://nosuchrepo/task/abc123'` opens the Open Work window on it.
- "Latest read failed." in the Files pane: open a started Task with its
  `loopflow://<repo name>/task/<id>` link, run `lf --task <id> desktop do files
  path=<file>` from its checkout, then `chmod 000` the checkout for a few
  seconds and restore it.
- A synthesized click does not give a SwiftUI `TextEditor` the keyboard (the
  backlog's brief editor), so text cannot be typed there; New Task's field
  works because the sheet opens with it focused.
- Moving a repository's `.git` away under an open window shows nothing: the
  page keeps its last read.
- Stopping the private server leaves its holder, the stand-in providers and any
  `lf task run` alive. List them with `ps -axo pid,ppid,command | grep
  <private dir>` and stop those PIDs, children first.
- Give the isolated `HOME` a `src/` directory, or the status line shows
  "Read …/home/src: No such file or directory".
- Launched while the private server is stopped, the app registers a launchd
  job for that Machine (`launchctl list | grep com.loopflow.server.`, the
  label ends in the socket directory's hash) and the server comes back on its
  own. To keep it stopped, `launchctl bootout gui/$(id -u)/<label>`; boot it
  out at the end too.
- Do not name the bare origin `<repo>.repo`: the repository conversation's
  checkout takes that path, and opening it fails with "git worktree add:
  worktree path already exists". (Useful when a real refusal is wanted.)
- Failure lines, each with the private server stopped (kill it, wait five
  seconds) unless said otherwise:
  - "…'s order not saved.": on a Wave's page (`lf wave create`, two `lf task
    create --wave`; a repository's backlog cannot reorder), select a row and
    press the up arrow. A read-only `.git` does not stop a rank.
  - "Some Work did not load.": server running, `lf desktop view 'wave:<real>
    or wave:nosuch'`. Three members give two tiles about 185 pt wide.
  - "Hidden paths were not saved.": a started Task's Diff tab, the sidebar
    toggle, a pattern typed into the "tests/, *.lock…" field, then stop the
    server and send Return.
  - "Diff settings did not load." and "Comparison unavailable.": stop the
    server, then open the Diff tab (from Files, with the sidebar open).
    Captures of this state take 20 to 30 seconds each.
  - "Conversation failed to open": select a repository or Wave whose
    conversation this launch has not opened, or press **Open conversation**.
  A broken `.lf/config.yaml` in the Task's checkout fails none of these.
- "File not read." in the Diff tab: `echo x > new.txt; chmod 000 new.txt` in
  a started Task's checkout, then open the tab. `chmod 644` and **Retry**
  reads it.
- "Skipped lines did not load." under a file's header: the Task's base needs
  a long file (commit one to the repository before `lf task create … --run`), two edits
  far apart in the checkout, then `chmod 000` the file and click an
  "N unchanged lines" row. The stream moves down when "Comparison
  unavailable." arrives, so capture first and take the row's position from
  that capture.
  A press within a second of the `chmod` can win the race and open the
  lines; press the file's other row. Open rows are gone after a relaunch.
- On an unlocked display that someone is using, launch with `open -n -g`:
  in `live` mode the window mounts behind the others and the capture loop
  and `lf desktop` work as they do locked. Starting a Task brings the app to
  the front once.
- A real screen capture of that window, title bar included, needs no focus:
  take the window number from `CGWindowListCopyWindowInfo` (owner name is
  the bundle's name, `kCGWindowIsOnscreen` 1) and run `screencapture -x -o
  -l <number> out.png`. This captures a `ScrollView` that meets the title bar.
- `lf desktop do files path=<file>` follows a file: with another file being
  read it opens a tab and leaves the selection. The Read/Diff control is a
  menu, so a file's own Diff in Files needs a local-only line setting
  `store.mode = .diff` after `presentFile`.
- After a `chmod` back, "Comparison unavailable." goes on its own in 12 to 40
  seconds; wait for it before reading the stream.
- A capture right after a change in the checkout can take 15 seconds or more
  to arrive; wait for its `.out` before reading the image.
- The run's quit script stops the QA app by the PID `pgrep -f` finds for the
  bundle's full executable path, never `pkill`.
- `lf desktop do details page=diff` fails with "The Task file location
  changed" in the window a Task was started in (Task ea8659); quit, relaunch
  and open the Task by its link first.
- Stop the run's processes by PID (`ps -axo pid,ppid,command | grep <private
  dir>`), never by pattern.

# Diff and Files without clicks

A headless run's shell may lack Accessibility and event-posting rights:
`osascript` keystrokes fail with error 1002, and CGEvent clicks and scrolls
silently do nothing. `lf desktop` still selects Work and pages:
`lf --task <id> desktop do details page=diff|files|brief`,
`lf --task <id> desktop zoom --on -p files` (the details pane) and
`lf --task <id> desktop do files path=<file>`. Capture just the app's window
with `screencapture -x -o -l <CGWindowID>`; get the ID from
`CGWindowListCopyWindowInfo` filtered on the owner name. The ID changes on
relaunch. "could not create image from window" means the display slept:
`caffeinate -u -t 1` wakes it.

The Diff can't be scrolled without input, so put the scenario first. Use a
disposable repository with a `qa` Graph (see the Task setup in the qa skill), run
the Task once for a checkout, then edit its working tree. The Diff includes
uncommitted changes, so one edited file at a time is the whole stream. The
store reads again on file events. Twice a list stayed stale after a `git
checkout` of one file plus an edit to another; the next write to any file
cleared it.

In-memory view state (the layout picker's `reviewLayout`, Source,
`TaskFilesStore.sourceFiles`) and the change stepper (`]`) have no
control command. Use the QA hook above to click Source, the layout picker
or the stepper in an instrumented build.

A Dev build whose served reads take new fields (`task.file`'s `at`) needs a
branch server. To use real Tasks, copy the store into a private home
(`sqlite3 ~/.lf/loopflow.db ".backup <home>/loopflow.db"`, same for
`history.db`, and copy `config.yaml`). Then fence it off: run
`update task_auto set state='off'` in the copy, put stand-in `claude`, `gh` and
`launchctl` first on `PATH`, and set `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY` to
`http://127.0.0.1:9`. Tracker tokens live in the keychain, not the home.
`<home>/logs/linear-requests.jsonl` should show only `"outcome":"failed"`.
Start `lf server run` from that environment and open the bundle with
`open -n --env …` for each variable. Building the bundle runs
`scripts/third_party.py package`, which fails when `swift/Package.swift`
changed without `uv run python scripts/third_party.py refresh`.
