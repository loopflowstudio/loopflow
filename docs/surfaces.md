# Surfaces

Your Tasks and conversations live in `lf`, not in a window. Open them wherever
you already work: Loopflow Desktop, cmux, Herdr, tmux or a plain terminal.

## Open Work where you are

```bash
lf open loopflow://loopflow/task/LOO-12        # LOO-12's conversation, in the host you're in
lf open loopflow://loopflow/wave/product       # the product Wave's conversation
lf open --available                            # every conversation available for you, at once
lf open loopflow://loopflow/task/LOO-12 --diff # conversation plus its Changes (Desktop)
```

From a Herdr pane, `lf open` adds a Herdr workspace; from tmux, a new window
right after this one; from cmux, a new workspace. Otherwise it opens in Desktop on
macOS, then a plain terminal. It reads only the host's own environment
(`HERDR_*`, `TMUX`, `CMUX_*`, Desktop's `LF_DESKTOP`) and never scans running
apps. `--host desktop|cmux|herdr|tmux|terminal` picks one explicitly.

A conversation already live in this host is focused, draft intact. One live in
another client is reported as `elsewhere`, with the `lf resume --takeover`
command to move it; opening never steals it. Open the same Task twice while its
workspace starts and you get the one workspace.

A Work address names the repository by folder, then the Task or Wave. It never
carries a path or a host, so the same link works on any machine with the repository.

## Share a link

```bash
lf --task LOO-12 open --link --format markdown   # a Markdown link to LOO-12's conversation
```

`--link` opens nothing. Paste it in a PR, a chat or a note; clicking it runs
`lf open` and lands in Desktop, or a terminal when Desktop isn't installed.
Installing `lf` registers a `loopflow:` handler, leaving one you chose in place.
With Herdr installed, Control-click opens the Work beside the clicked pane.

## Loopflow Desktop

The Mac app shows the same records the commands print; it keeps no copy of its own.

- **Working Tasks**: the sidebar, ⌘1–9 in order.
- **Task workspace**: three panes, the `session` conversation, a `shell` at the
  Task's checkout and a `browser` for Files, the Graph log and local pages.
- **Dashboards**: Tasks side by side, chosen by a query.
- **Wave view**: the Wave conversation beside a standup board, with the Tasks
  being worked on three to a page below. The board shows one section at a time:
  Backlog, Working (with each Session Available or Busy), Autonomous (shipping)
  and Done. The repository page is the same view over every repository Task.
  Every pane and view part zooms and hides from buttons on hover, its dividers
  drag, and the header's `Show…` menu brings hidden parts back.
- **⌘K**: search Waves, Tasks, Sessions and Graphs.

```bash
lf desktop view 'state:available'                       # a Dashboard of every available conversation
lf desktop query attention 'state:available or state:blocked'   # save it as "attention"
```

Queries take `wave:`, `task:`, `kind:task|wave` and
`state:open|available|running|blocked|paused|idle|backlog|done`; whitespace
means AND, and `or` and parentheses compose.

## Drive Desktop from a script

`lf desktop` is Desktop's own pane API, the same kind of thing as tmux's,
cmux's, Herdr's or Rex's. Use theirs directly in those hosts; use this one in
Desktop. Every key and menu item is a verb here by the same name.

```bash
lf desktop run 'cargo test -p loopflow'           # a new shell tab in the Task's checkout; focus stays put
lf desktop wait -p shell:2 --for 5m               # exits with that test run's status
lf desktop capture -p shell:2 --trim | tail -3
lf --task LOO-12 desktop send -p session 'also cover the timeout path'   # into the agent's draft, not sent
lf desktop browse http://localhost:5173
lf desktop move -p browser --destination session --axis vertical
lf desktop goto 2                                 # what ⌘2 does
lf desktop do section -p board section=working    # the Wave board: backlog, working, autonomous or done
lf desktop do filter -p board days=14             # Done covers 14 days; kept on this Mac
lf desktop zoom --on -p board                     # the board fills the view; --off returns
```

Without `-p`, a command acts on the terminal it runs in, so an agent's own tool
call reaches its own pane. `-p` is a path on the screen: a pane (`session`,
`shell`, `browser`), a Wave view part (`board`, `tiles`), a tab (`shell:2`), a
Dashboard tile (`3`, `LOO-12`) or `TILE/PANE`. `lf desktop state` lists each
view's parts under `view_parts`, with their ratio, hidden and zoomed state. Each part matches full ID, then label, then position, then an ID
suffix. Nothing follows focus: an ambiguous target lists the choices and changes nothing.

`send` types without submitting; `send-key enter` submits. `run` types into a
real shell tab, so the tab outlives the command.

## Build your own view

Every read is JSON, and the shapes are published:

```bash
lf monitor --json                         # waiting, blocked, active and finished work, with next actions
lf task list --json                       # open Tasks, as your sidebar would list them
lf task status LOO-12 --json              # one Task: its plan, PR, Graph steps and conversations
lf session list --json                    # conversations, with which ones are available for you
lf desktop view 'wave:product' --plan     # what a Dashboard would show, for your own host; opens nothing
lf --task LOO-12 open --link --json       # the target, its fallback command and the route it would take
lf desktop events --json                  # Desktop's window and pane changes, one line each
lf help desktop --all --json              # every Desktop command's arguments and result schema
```

`desktop view --plan`, `query`, `pin`, `rename` and `layout` work without macOS
or a running Desktop; they read and write the same saved Dashboards Desktop
uses. A program running in a Desktop shell can report its own state with
Program Status (OSC 7501, Rex's protocol); a blocked report marks the pane available.

## Reference

| `lf desktop` exit | Means |
|---|---|
| 0 | Done |
| 1 | Failed, or Desktop speaks another API version (update it) |
| 2 | Bad arguments or expression |
| 3 | Task, window, pane or tab missing or ambiguous; nothing changed |
| 4 | Desktop unreachable: not running, not macOS, or not answering |

`lf desktop actions` lists every verb with its arguments and default keys. Full
arguments: [`lf open`](lf-reference.md#lf-open), [`lf desktop`](lf-reference.md#lf-desktop).

Next: [Telemetry](telemetry.md), what your agents did and what it cost.
