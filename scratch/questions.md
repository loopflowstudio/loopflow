# LOO-519 questions and assumptions

Jack Heart waived design review (2026-10-24); kickoff settled these, Jack judges
them in the demo. Each is reversible before landing.

- **No `LF_TASK`/`LF_SESSION`/`LF_REPO` (accepted, Jack Heart 2026-10-24).** Caller
  context is `LF_TERMINAL_ID`, trusted for targeting; the earlier
  `LF_AGENT_CALLER` Session matching is dropped.
- **Other conversations of a Task switch into the session pane**
  (`do session.switch`), not browser tabs. Keeps browser = documents/web and the
  session pane a single slot; Desktop retains switched-away surfaces.
- **Side-by-side shells: out.** One visible shell; revisit if it bites (a pane
  showing two of its own tabs, not a split tree).
- **Browser gets a real web view now** (`WKWebView` URL tabs, http/https), beside
  `files` and `graph-log` tabs. Without it "browser" misnames the pane and the
  dev-server demo has nowhere to land. Files keeps its own document tabs inside
  its tab this Task.
- **`ls` and `state` both exist; one inspection.** `ls` is Rex's name and a
  rendering of the same Apple-event reply; `state --json` stays the oracle.
  `ls` scope: the caller's window, else the frontmost one, named in the header.
- **Task flag is `-t/--task`, not Rex's `-s`**, because Session means a
  conversation in Loopflow. No `-w` (a Task has one workspace); Desktop window is
  `--window` (long only; Rex's `-C` is uppercase, which lf avoids).
- **`run` takes the Rex name for "new shell tab"** and accepts `-- CMD`; `--wait`
  stays with LOO-453.
- **Bare target never follows focus.** Without caller or binding, more than one
  candidate exits 3.
- **`view working-tasks` is deleted** in favor of `do dashboard.working_tasks`.
- **Layout migration converts stored layouts** (Files+Log → one browser pane,
  empty panes collapse, extra Session panes leave the layout). No dual-shape reader.
- **Doubled Dashboard frame — Jack's call in the demo.** `DashboardView.swift` and
  `WorkspacePanesView.swift` both stroke `palette.border`. No change planned.
- **Close hides; ⌘W closes the selected tab (implemented 2026-10-24).** With
  retained panes, ⌘W on a shell or browser closes its selected tab and the last
  tab hides the pane; on the session pane it hides the pane. Undo shows the same
  occurrence again (the old "Undo never replays" rule holds because the shell is
  never torn down).
- **New layouts use pane IDs `session`, `shell`, `browser`.** Migrated layouts
  keep their stored IDs; `-b session` resolves by kind either way.
- **Action request has no `task` field (implement, 2026-10-24).** Every Work-scoped
  action addresses a pane, and `DesktopPaneTarget` already carries the Work.
- **⌘W and ⇧⌘T are their own actions.** `pane.close` closes the selected tab (last
  tab hides the pane); `pane.reopen` undoes the last close. `pane.hide`/`pane.restore`
  act on the addressed pane only, so scripts never undo something else.
- **⇧⌘E is `browser.toggle`.** It shows or hides the browser pane (opening Files
  when it has no tab), replacing "Toggle Files" now that Files is a browser tab.
- **Root `--task`/`--wave` outrank the caller's own pane.** The plan listed the
  caller first; an explicit root flag from a Desktop shell should win. `-t` wins
  over both.
- **Window positions use inspection order, not frontmost.** The inspection has no
  z-order yet; `ls` with several windows and no caller exits 3 asking for
  `--window`. Frontmost ordering is a Swift change in `WorkLinkRouter.inspect`.
- **`run -- CMD` runs through `/bin/sh -c`** with the argv shell-quoted by lf.
- **Crumb chevrons and the sidebar's Working Tasks item still call `WorkModel`
  directly** (`goBack`, `selectWorkingTasksDashboard`); the registry's handlers call
  the same functions. Routing chrome clicks through the registry is optional.
- **Proposed Wave memory edit (not applied):** `.lf/wave/product/MEMORY.md:138`
  still describes `--pane` and verified caller-terminal inference.
