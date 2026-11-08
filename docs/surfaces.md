# Surfaces

Your Tasks and conversations live in `lf`, not in a window. Open them wherever
you already work: Loopflow Desktop, Rex, cmux, Herdr, tmux or a plain terminal.

For Task layouts, Wave defaults, automatic arrivals and native splits, read
[Open Task workspaces](workspaces.md).

## Open Work where you are

```bash
lf open loopflow://loopflow/task/LOO-12        # LOO-12's conversation, in the host you're in
lf open loopflow://loopflow/wave/product       # the product Wave
lf open --available                            # every conversation available for you, at once
lf open loopflow://loopflow/task/LOO-12 --diff # conversation plus its Changes (Desktop)
```

From Rex, Herdr or cmux, `lf open` launches a fresh Task tab with its splits in
the host you are in: inside a Rex session or Herdr workspace, a Task joins it; a
repository or Wave gets its own session, workspace or cmux group. Each open is a
new launch; return to earlier tabs through the host. From tmux it opens a new
window right after this one. Otherwise it opens in Desktop on macOS, then a plain
terminal. Host context comes from viewer placement, the host's environment or Rex
process ancestry; opening never scans running apps, and failure in your host never
falls through to another. `--host desktop|rex|cmux|herdr|tmux|terminal` picks one
explicitly.

For a selected Task, Rex, Herdr and cmux run `lf resume` in its Conversation pane,
which moves the conversation's input here. A repository or Wave opening starts no
agents: it reconnects running conversations no terminal is showing and shows
workflow information for the rest, as automatic Task arrivals do. tmux and terminal
routes focus an existing conversation or report it as `elsewhere`.

A Work address names the repository by folder, then the Task or Wave. It never
carries a path or a host, so the same link works on any machine with the repository.

## Present documentation in Desktop

```bash
lf open --host desktop --file docs/usage.md --file README.md
```

Present the original files in an attached Desktop Files view, without starting a
host or conversation. Paths are relative to the selected checkout. Markdown and
HTML render in read mode; active edits keep their selection and draft while
another file becomes a tab. Desktop retains its existing remote Files transport.

With no Desktop attached, the command reports paths and opens nothing. A missing
file, invalid Work selection or unmatched view reports an error; correct the
path or destination and retry presentation without rewriting the docs. The
docs-first workflow reports paths and continues when presentation is unavailable.
It never launches a terminal editor, browser or host as a fallback.

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
lf desktop dashboard query attention 'state:available or state:blocked'   # save it as "attention"
```

Queries take `wave:`, `task:`, `kind:task|wave` and
`state:open|available|running|blocked|paused|idle|backlog|done`; whitespace
means AND, and `or` and parentheses compose.

### Connection recovery

Desktop uses the local Loopflow server to read Work and attach Session terminals.
Installation prepares the connection. Check availability with `lf server status`.
The native connection row shows recovery while keeping the screen and local
reading actions available; input waits for acknowledgement. **Retry** reconnects;
**Move here** explicitly requests control when another viewer has it. Recovery
never replays uncertain input. See [Connect clients](server.md) for updates,
background access and the distinction from Machine execution destinations.

## Drive Desktop from a script

`lf desktop` is Desktop's own pane API, the same kind of thing as tmux's,
cmux's, Herdr's or Rex's. Use theirs directly in those hosts; use this one in
Desktop. Every key and menu item is a verb here by the same name. This automation
endpoint lives with the app; the client server has an independent lifetime.

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

A herdr, tmux or other GUI pane uses the reads below. Each streams with
`--subscribe --json`: one JSON line now, another each time its answer changes,
until stdin closes.

```bash
lf wave show --all --subscribe --json         # every Wave's plan and Tasks
lf task list --subscribe --json               # open Tasks, as your sidebar would list them
lf task status LOO-12 --subscribe --json      # one Task with its conversations, Graph processes and thread
lf session list --subscribe --json            # conversations, with which ones are available for you
lf session history --since 7d --subscribe --json   # what Sessions did, tagged with their Task and Wave
lf process list --subscribe --json                      # live lf call trees
lf repo plan --subscribe --json         # planning sync; keeps it exchanged while it runs
lf desktop view list --subscribe --json           # every plan a repository window shows
```

```json
{"answers": null, "unavailable": null, "body": {…}}
```

Every Task list read (`lf task list`, `lf wave show`, `lf wave status`,
`lf plan list`) prints the same Task record the server sends:

```json
{"task": {"id": "task_…", "identifier": "LOO-12", "name": "Search transcripts", "wave_id": "…", …},
 "runtime": {"status": "active", "reason": "…", "owner": {…}, …},
 "next_move": {"owner": "task", "reason": "…"},
 "condition": {"state": "clear", "reason": "…", …},
 "graph_name": "code", "pr": {"phase": "open", …}, "section": "now", …}
```

`runtime.status` is one of `unstarted`, `idle`, `active`, `moving`,
`shipping`, `done` or `abandoned` (`runtime` is `null` before a first run);
`next_move.owner` (`user`, `wave`, `task`, `ci`, `external`) says who moves it
on. `lf task status <issue> --json` prints one
Task as a row, each fact once, with its Sessions, Graph processes and
thread.

A Session record:

```json
{"id": "session_…", "title": "Search transcripts", "provider": "claude", "interactive": true,
 "state": "active", "availability": "available", "work": {"kind": "task", "id": "task_…"},
 "task_ids": ["task_…"], "wave_id": "…", "cwd": "/src/loopflow.loo-12", "actions": [{"kind": "open", …}], …}
```

`state` is what its agent process shows (`active`, `detached`, `closed`,
`unknown`); `availability` is `"available"` when it is ready for your next
turn and `null` while it works. `lf resume <id>` opens it.

Desktop reads the same records through `lf plan list`, `lf task show`,
`lf wave plan`, `lf task list --closed` and `lf session list`, with the
same `--subscribe`. `lf wave status` and `lf wave show` exchange planning with the
tracker first; `lf wave plan` and `lf plan list` read what the store
records, and `lf wave sync` refreshes it.

`body` is what the same command prints with `--json` alone. Write
`{"id": 1, "args": {"query": "parser"}}` to stdin to change what it reads; the
next line answers `1`. The protocol is in [Work](work.md#see-where-it-stands).
A development build with unreleased migrations reads a preview copy; its
`lf --version` adds `copy: <directory>`.

One-shot reads for everything else:

```bash
lf desktop view show 'wave:product'     # what a Dashboard would show, for your own host; opens nothing
lf --task LOO-12 open --link --json       # the target, its fallback command and the route it would take
lf desktop events --json                  # Desktop's window and pane changes, one line each
lf help desktop --all --json              # every Desktop command's arguments and result schema
```

`desktop view show`, `desktop dashboard` and `desktop layout` work without macOS
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
