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
