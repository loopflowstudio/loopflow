# LOO-519 questions and assumptions

Jack Heart waived design review (2026-10-24); kickoff settled these, Jack judges
them in the demo. Each is reversible before landing. v1 entries the v2 direction
superseded (`-t`/`-b`, `do`, `run -- CMD`, layout conversion) were removed at
realign, 2026-10-25; see git history.

- **No `LF_TASK`/`LF_SESSION`/`LF_REPO` (accepted, Jack Heart 2026-10-24).** Caller
  context is `LF_TERMINAL_ID`, trusted for targeting; the earlier
  `LF_AGENT_CALLER` Session matching is dropped.
- **Other conversations of a Task switch into the session pane**
  (`switch`), not browser tabs. Keeps browser = documents/web and the
  session pane a single slot; Desktop retains switched-away surfaces.
- **Side-by-side shells: out.** One visible shell; revisit if it bites (a pane
  showing two of its own tabs, not a split tree).
- **Browser gets a real web view now** (`WKWebView` URL tabs, http/https), beside
  `files` and `graph-log` tabs. Without it "browser" misnames the pane and the
  dev-server demo has nowhere to land. Files keeps its own document tabs inside
  its tab this Task.
- **`ls` and `state` both exist; one inspection.** `ls` is Rex's name and a
  rendering of the same `inspect` reply; `state --json` stays the oracle.
  `ls` scope: `--window`, else the caller's window, else the repository's only
  window (see window positions below). The Task asked to "keep one command";
  `list` is gone and `ls` renders `state`'s reply, so there is one readback.
- **Bare target never follows focus.** Without caller or binding, more than one
  candidate exits 3.
- **Doubled Dashboard frame — Jack's call in the demo.** `DashboardView.swift` and
  `WorkspacePanesView.swift` both stroke `palette.border`. No change planned.
- **Close hides; ⌘W closes the addressed tab (implemented 2026-10-24).** ⌘W on a
  shell or browser closes the addressed (else selected) tab and the last tab hides
  the pane; on the session pane it hides the pane. ⇧⌘T ("Undo Close") shows the same
  occurrence again: a closed shell tab's process stays alive until the next close
  replaces the undo record (implemented, slice 5). The alternative is that ⌘W ends
  the shell immediately and ⇧⌘T only shows hidden panes. Jack judges which one in
  the demo.
- **Action request has no `task` field (implement, 2026-10-24).** Every Work-scoped
  action addresses a pane, and `DesktopPaneTarget` already carries the Work.
- **⌘W and ⇧⌘T are their own actions.** `close` closes the addressed tab (last
  tab hides the pane); `reopen` undoes the last close. `hide`/`show`
  act on the addressed pane only, so scripts never undo something else.
- **⇧⌘E is `toggle-browser`.** It shows or hides the browser pane (opening Files
  when it has no tab), replacing "Toggle Files" now that Files is a browser tab.
- **Window positions use inspection order, not frontmost.** The inspection has no
  z-order yet; `ls` with several windows and no caller exits 3 asking for
  `--window`. Frontmost ordering is a Swift change in `WorkLinkRouter.inspect`.
- **Crumb chevrons and the sidebar's Working Tasks item still call `WorkModel`
  directly** (`goBack`, `selectWorkingTasksDashboard`); the registry's handlers call
  the same functions. Routing chrome clicks through the registry is optional.
## Concept review, 2026-10-25 (Jack Heart, in the Task conversation)

- **Unresolved: pane visibility vs tab closing.** Hiding a pane has five spellings
  (`hide`, `toggle-shell`, `toggle-browser`, ⌘W on session, ⌘W on a last tab) and
  getting it back depends on which was used; `state` does not say. Proposed, not
  accepted: panes only hide/show, tabs only close/reopen, closing the last tab leaves
  an empty pane, ⌘W on the session pane does nothing, `toggle-shell`/`toggle-browser`
  go. Question for Jack: when the last shell tab closes, does the pane stay visible
  and empty, or vanish? It also settles whether ⌘W ends the shell.
- **Follow-up Tasks filed at Jack's request:** LOO-590 (PR as a browser tab),
  LOO-591 (Task state in its workspace), LOO-592 (file browser inside the browser pane).

## Implement v2, 2026-10-25 (assumptions, reversible)

- **`wait` semantics.** On a shell tab: done when every line `run` submitted has
  finished (exit with the last status), else at the next finish after the wait
  began. On a pane: done when it is ready (session surface running; browser at
  once). A closed tab has finished. `--for` elapsing exits 1. Depends on the shell's
  OSC 133 integration; a shell without it never finishes.
- **`events` uses a 250 ms coalesced poll** inside Desktop, only while a client is
  subscribed, comparing each window's encoded reading. Desktop state spans
  Observation, ObservableObject and AppKit; a push signal is LOO-593's to design.
- **Toggles are key-only.** `toggle-shell`/`toggle-browser` stay in the registry for
  their keys but have no CLI verb; `actions` still lists them.
- **`browse URL` names "open a web page"** because `lf desktop open` would collide
  with `lf open` shorthand. Page history is `page-back`/`page-forward`, distinct
  from window `back`/`forward`.
- **On a Dashboard a lone `-p` part is a tile first, then a pane** of the default
  Task; on a Task view it is a pane, and `TILE/` picks a sidebar Task.
- **Old layouts are deleted, not converted** (Jack: no migration of existing data);
  file drafts stored in them go too.
- **`LF_DESKTOP=1` removed here** (v2 says it goes) rather than in LOO-571; LOO-571
  can close with this PR.
- **`lf wait` shorthand docs now say `lf task wait`**: `lf desktop wait` makes the
  shorthand ambiguous.

- **No input from Jack (2026-10-25)** on: last shell tab closed → empty pane or hidden;
  Dashboard `resize --span`. Keep what is implemented (closing the last tab hides the
  pane; Dashboard sizing stays in `layout`) and don't raise them again unless the demo
  shows a problem.

## Realign, 2026-10-25

- **Proposed Wave memory edit (not applied; `.lf/wave/` is read-only outside a Graph
  that maintains it).** `.lf/wave/product/MEMORY.md` (LOO-427 paragraph, ~line 136)
  still teaches `--repo`/`--task`/`--pane` and PTY-verified caller inference. Replace
  with: `lf desktop` talks to a socket inside Desktop
  (`$LF_HOME/run/desktop/<bundle-id>.sock`); targets resolve root `--task`/`--wave`,
  trusted `LF_TERMINAL_ID`, Task binding, in Rex order (ID, label, position, suffix);
  `-p` paths address panes in Task, Dashboard and Wave views; registry action names
  are the verbs; exit 2/3/4 from typed error codes. Memory is at its token limit
  (15996/16000), so the replacement must not grow the paragraph.

## Implement slice 7, 2026-10-25 (assumptions, reversible)

- **lf writes tile edits; Desktop only follows.** Desktop's socket `perform` is
  synchronous and its record writes already shell out to lf, so a Desktop-side tile
  action would call back into lf. The verbs resolve the tile from the live reading,
  write through the store owners, and build the `applies_to: dashboard` receipt from
  that reading; Desktop catches up through its Work stream.
- **Tile edits use the current revision.** Hide/show/move are deltas on one member,
  so a stale read cannot clobber another edit. Whole slot layouts keep `--revision`.
- **`slots` folded into `layout query:ID`, not `resize`.** The slot editor adds,
  removes and swaps slots, which no tile verb expresses; `resize` reweights the
  innermost split between two tiles.
- **Working Tasks and Wave views only `move`** (the shared Task order); hide, show and
  resize there exit 1 and say to save a Dashboard.
- **Wave view keeps its inventory** under the tiles; tiles never take window focus
  (the window would otherwise switch to the Task). LOO-594 owns the real layout.
