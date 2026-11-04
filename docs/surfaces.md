# Surfaces

Your Tasks and conversations live in `lf`, not in a window. Open them wherever
you already work: Loopflow Desktop, Rex, cmux, Herdr, tmux or a plain terminal.

## Open Work where you are

```bash
lf open loopflow://loopflow/task/LOO-12        # LOO-12's conversation, in the host you're in
lf open loopflow://loopflow/wave/product       # the product Wave
lf open --available                            # every conversation available for you, at once
lf open loopflow://loopflow/task/LOO-12 --diff # conversation plus its Changes (Desktop)
```

From Rex, `lf open` opens a native workspace for the selected Work. It uses
recorded viewer placement when available, otherwise Rex's configured local
server and caller block. Custom-server discovery and the complete agent-tool
path remain unfinished; known Rex failures never fall through to Desktop.
From a Herdr pane, `lf open --task LOO-12` adds a Herdr workspace; from tmux, a new window
right after this one; from cmux, a new workspace. Otherwise it opens in Desktop on
macOS, then a plain terminal. It reads only the host's own environment
(`REX_*`, `HERDR_*`, `TMUX`, `CMUX_*`, Desktop's `LF_DESKTOP`) and never scans running
apps. `--host desktop|rex|cmux|herdr|tmux|terminal` picks one explicitly.

Rex and Herdr open passive conversation views. Other terminal routes focus an
existing conversation in that host or report it as `elsewhere`, with the
`lf resume` command that moves input. Opening never steals input. Repeated
opening reuses the workspace.

A Work address names the repository by folder, then the Task or Wave. It never
carries a path or a host, so the same link works on any machine with the repository.

## Rex workspaces

```bash
lf open loopflow://loopflow --host rex  # repository's started Tasks, grouped by Wave
lf open --host rex --wave product      # the Wave's working set
lf open --host rex --task LOO-12        # one Task; no sibling arrivals
lf open --host rex --task LOO-12 --copy review
lf task view LOO-12                    # brief, workflow, Sessions and step results
```

Rex owns its splits and ordinary shells. Reopening reuses native windows without
rerunning their tools or changing input control. Conversation viewers observe
existing LF Sessions; `lf resume <session>` explicitly takes input or resumes saved
provider history. An observing copy starts no new ordinary shells. Add
`--new-shells` when creating a copy to include fresh shells; receipts count them.
The Task viewer uses `n`/`p` to select a step and `l` to inspect its captured log.
Without a conversation, the builtin layout shows Task & workflow beside a shell.
Explicitly opening a passive arrival adds its tools beside the existing panes
once. Reopening a broad workspace recovers an exited follower in a new tab,
preserving native edits in its old tab.

New Task windows use saved Task pane intent, then the Wave's default, then the
builtin arrangement. Desktop and Rex read the same semantic tools; their active
layouts remain separate. Only selected, visible tabs materialize. File drafts,
URL tabs and background tabs remain in the saved source layout. A selected web
page has no terminal renderer and is reported as unsupported.

The Workspace tab follows local Work changes. A Task's first start adds a
passive Task view without taking focus or starting a provider or shell. Closing
that tab stops following; a later explicit open recovers following and reconciles
missed arrivals. Closing a Task tab leaves it closed during following. Missing
observations never retire work.
Passive views start in their Task's recorded checkout.

```bash
lf desktop layout wave-default:<wave-id>             # read the default
lf desktop layout wave-default:<wave-id> --save < layout.json
lf task tool LOO-12 files --checkout /recorded/checkout
lf task tool LOO-12 changes --checkout /recorded/checkout --base parent --difftool
```

Wave defaults contain `revision`, the existing pane `tree`, and `hidden` pane IDs.
Templates select tools without capturing Sessions, execution locations or file
drafts. Changes tabs carry a `comparison`: `working_tree` with `base`, `branch`
with `base`/`head` SHAs, or `pull_request` with those SHAs and its `number`. Edits affect future Task materialization; saved Task layouts win.
Files uses `LF_FILE_BROWSER` for directories, then `VISUAL`, `EDITOR`, or `vi`.
Changes uses Git's pager for the Task's patch against the selected base, including
untracked files. `--difftool` delegates tracked-file comparisons to Git's configured
diff tool.
Branch and PR intents pin both commit SHAs; `--head <sha>` reads that committed
comparison without mixing in local changes. Editor settings may launch an
external GUI. Paths remain separate arguments.

`--server` selects a local Rex Unix server explicitly. Remote Tasks open through
their recorded Machine's LF route and local Rex server, in separate native
containers. Unavailable Machines stay unavailable; no local shell substitutes.
`--no-focus` preserves the host's selected window.
A successful server mutation does not establish app visibility or provider input
readiness.

The accepted workspace experience still requires these unfinished behaviors:

| Required behavior | Current limit |
| --- | --- |
| Native splits start at the selected Task's checkout on its Machine | LF-created panes have the right cwd; the inspected Rex native API starts later splits in home when cwd is omitted. No scoped split default is available in that API. |
| Reopen restores familiar tools after a server restart | Rex can retain block IDs but replace their commands with login shells. LF preserves those blocks; it cannot safely infer permission to replay commands. |
| Bare open finds the current Rex server through shell and agent calls | A caller on a non-default server still needs `--server` unless viewer placement supplies its endpoint. Full agent invocation after resume/move remains unverified. |
| Broad workspaces follow Tasks on every relevant Machine | Explicit open routes remote Tasks; automatic arrival currently follows local Tasks only. |
| Hosts retain and release Sessions through shared cleanup | Rex occurrence reporting and scoped native closure are not integrated yet. Opening alone registers no lifetime hold; unknown observations authorize no cleanup. |

Herdr support is bounded to a selected Task on the current local server,
identified by `HERDR_SOCKET_PATH`: create, reuse and focus a workspace, with direct
argv layouts and existing Session observation. It does not implement Wave following,
remote forwarding or live process migration. Native close removes only that
workspace; an uncertain creation is retained for inspection before retry.

## Open files where you are

```bash
lf open --file docs/usage.md --file README.md
```

Open the actual files in the current host without starting a conversation.
Paths are relative to the selected checkout. Desktop uses its Files view;
Rex, Herdr, cmux and tmux open the Files editor alongside existing work.
Terminal editors follow `VISUAL`, then `EDITOR`, then `vi`. Existing panes and
unsaved edits remain in place. No attached host means a successful no-op; the
command never launches a host as a fallback.
Terminal file presentation currently requires a local checkout; Desktop Files
also supports Tasks on another Machine.

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

A herdr, tmux or other GUI pane uses the reads below. Each streams with
`--watch --json`: one JSON line now, another each time its answer changes,
until stdin closes.

```bash
lf wave show --all --watch --json         # every Wave's plan and Tasks
lf task list --watch --json               # open Tasks, as your sidebar would list them
lf task status LOO-12 --watch --json      # one Task with its conversations, Graph processes and comments
lf session list --watch --json            # conversations, with which ones are available for you
lf history --since 7d --watch --json      # what Sessions did, tagged with their Task and Wave
lf ps --watch --json                      # live lf call trees
lf planning status --watch --json         # planning sync; keeps it exchanged while it runs
lf desktop plans --watch --json           # every plan a repository window shows
```

```json
{"answers": null, "unavailable": null, "body": {…}}
```

Every Task read prints the same Task row, each fact once:

```json
{"id": "task_…", "identifier": "LOO-12", "title": "Search transcripts", "wave": "infra",
 "project": "Search", "state": "active", "reason": "…", "workflow": {"name": "code", "stage": "review"},
 "pr": {"number": 1719, "url": "…", "phase": "open"}, "next": "lf task run LOO-12"}
```

`state` is one of `unstarted`, `idle`, `active`, `moving`,
`shipping`, `done` or `abandoned`; `next` is the command that moves the Task
on, `null` while it moves by itself.

A Session row:

```json
{"id": "session_…", "title": "Search transcripts", "provider": "claude", "interactive": true,
 "state": "active", "available": true, "task": "task_…", "wave": "infra",
 "cwd": "/src/loopflow.loo-12", "agent_process_id": "…"}
```

`state` is what its agent process shows (`active`, `detached`, `closed`,
`unknown`); `available` says it is ready for your next turn. `lf resume <id>`
opens it.

Desktop's own presentation (sections, conditions, controls) is
`lf desktop plan`, `lf desktop task`, `lf desktop wave`, `lf desktop done` and
`lf desktop sessions`, with the same `--watch`.

`body` is what the same command prints with `--json` alone. Write
`{"id": 1, "args": {"query": "parser"}}` to stdin to change what it reads; the
next line answers `1`. The protocol is in [Work](work.md#see-where-it-stands).
A development build with unreleased migrations reads a preview copy; its
`lf --version` adds `copy: <directory>`.

One-shot reads for everything else:

```bash
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
