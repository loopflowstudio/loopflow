# Surfaces

Your Tasks and conversations live in `lf`, not in a window. Open
them wherever you already work: Loopflow Desktop, Rex, cmux, Herdr, tmux, or
any plain terminal.

For presentation modes, sibling tabs, and workspace layouts, see
[Workspaces](workspaces.md).

## Open Work anywhere

```bash
lf open loopflow://loopflow/task/LOO-12       # attach LOO-12's conversation here
lf open --task LOO-12 --tab                   # attach in a sibling tab
lf open --task LOO-12 --focus                 # reveal its existing view
lf open --wave product                        # open the Wave's conversation
lf open --wave product --workspace            # open the Wave's Task overview
lf open --task LOO-12 --host desktop          # open in Loopflow Desktop
```

- Direct CLI calls attach in the current terminal (including inside Rex). Agent
  tool calls default to a sibling tab and never attach inside their own tool
  output.
- Multiple views of the same conversation can be open at once, and every view
  can type. `--focus` reveals an existing view without opening a new one (or
  reports **Not open**).
- `--host desktop|rex|cmux|herdr|tmux|terminal` picks the host explicitly; a
  failure never silently falls back to another host.
- `loopflow://<repo>/task/<ID>` links carry no local path or hostname, so they
  work on any machine that has the repo. Generate a Markdown link with
  `lf --task LOO-12 open --link --format markdown` (paste into PRs, Slack, or
  notes; installing `lf` registers the `loopflow:` URL handler).

To present docs in Desktop:

```bash
lf open --host desktop --file docs/usage.md --file README.md
```

Opens those files in Desktop's Files pane (rendered Markdown/HTML, keeping
active drafts intact) without launching a conversation or editor.

`lf session rename SESSION 'Resume repair'` updates the
terminal title via standard OSC sequences, plus native cmux tabs/workspaces and
tmux windows. Rex and Herdr users can configure title display in those apps'
own settings.

## Loopflow Desktop

The native macOS app reads the same local store as `lf`; it keeps no copy of its own:

- **Sidebar (⌘1–9):** your working Tasks in shared order (dragging reorders
  them for CLI lists too).
- **Task workspace:** three resizable panes: `session` (conversation), `shell`
  (at the Task's worktree), and `browser` (Files, Graph log, local dev URLs).
- **Dashboards:** saved side-by-side grids of Tasks selected by query.
- **Wave and repository views:** the scope's conversation beside a standup
  board (`Backlog`, `Working`, `Done`) and Task tiles below; a Task with auto
  on carries an Auto badge.
  Every pane zooms and hides on hover; the header's `Show…` menu restores
  hidden parts.
- **⌘K:** jump to any Wave, Task, Session, or Graph.

Save and open Dashboards from the CLI too:

```bash
lf desktop view 'state:available'
lf desktop dashboard query attention 'state:available or state:blocked'
```

Queries support `wave:`, `task:`, `kind:task|wave`, and
`state:open|available|busy|blocked|unstarted|idle|active|moving|backlog|working|done|auto`
(`auto`: an open Task with auto on), combined with whitespace (AND), `or`, and parentheses.

### Changes show at once

Actions in Desktop (Complete, Abandon, Reopen, Claim, Unassign, Move, Rename)
update the UI on the click, including for remote Machines and the phone. If the
Machine refuses, the row returns and a single bottom Notice explains why
(auto-dismissing after 6 seconds; the **Auto** toggle's Notice carries **Undo**,
which turns auto off).

If the Machine pauses briefly, edits still show immediately and flush once it
reconnects. After ~10 seconds offline, mutation controls disable until the
connection returns. See [Connect clients](server.md) for connection recovery.

## Script Desktop

`lf desktop` is Desktop's pane and window API, the equivalent of tmux's or
cmux's CLI:

```bash
lf desktop run 'cargo test -p loopflow'           # new shell tab in this Task's worktree
lf desktop wait -p shell:2 --for 5m               # wait and exit with that command's status
lf desktop capture -p shell:2 --trim | tail -3    # read terminal output
lf --task LOO-12 desktop send -p session 'also cover the timeout path'   # draft text, not sent
lf desktop browse http://localhost:5173
lf desktop move -p browser --destination session --axis vertical
lf desktop goto 2                                 # same as ⌘2
lf desktop do section -p board section=working    # switch Wave board section
lf desktop do filter -p board days=14             # Done window (kept on this Mac)
lf desktop zoom --on -p board                     # zoom a pane; --off restores
```

Without `-p`, commands act on the terminal they ran in. `-p` targets a pane
(`session`, `shell`, `browser`), Wave view part (`board`, `tiles`), tab
(`shell:2`), or Dashboard tile (`3`, `LOO-12`, `TILE/PANE`). `lf desktop state`
lists every part. `send` types without submitting; `send-key enter` submits.

## Build your own view

Every surface, including Desktop, tmux plugins and custom GUIs, reads the
same `--subscribe --json` streams (NDJSON, one line on every change until stdin
closes):

```bash
lf wave show --all --subscribe --json
lf task list --subscribe --json
lf task status LOO-12 --subscribe --json
lf session list --subscribe --json
lf session history --since 7d --subscribe --json
lf repo tracker --subscribe --json
lf desktop view list --subscribe --json
```

Each line is `{"answers": null, "unavailable": null, "body": {…}}`, where
`body` matches `--json` alone (send `{"id": 1, "args": {…}}` on stdin to update
filters; see [Work](work.md#see-where-everything-stands)).

**Task record shape** (shared by `lf task list`, `lf wave show`, `lf wave status`, `lf plan list`):

```json
{"task": {"id": "task_…", "identifier": "LOO-12", "name": "Search transcripts", "wave_id": "…"},
 "runtime": {"status": "active", "reason": "…"},
 "next_move": {"owner": "task", "reason": "…"},
 "condition": {"state": "clear", "reason": "…"},
 "graph_name": "code", "pr": {"phase": "open"}, "section": "working",
 "lifecycle": "active", "selection_states": ["open", "busy", "active", "working"]}
```

**Session record shape** (`lf session list --json`):

```json
{"id": "session_…", "title": "Search transcripts", "provider": "claude", "interactive": true,
 "state": "active", "availability": "available", "work": {"kind": "task", "id": "task_…"},
 "cwd": "/src/loopflow.loo-12",
 "turn": {"id": "5b0c…", "state": "ended", "started_at": "2026-11-10T16:25:37Z",
          "ended_at": "2026-11-10T16:27:51Z", "model": "claude-opus-5-5",
          "usage": {"input_tokens": 41200, "cached_input_tokens": 38000, "output_tokens": 1100}}}
```

`availability` is `"available"` when ready for your next turn and `null` while
working. Raw provider sessions started outside `lf` appear with `"raw": true`
and their provider conversation ID.

One-shot helpers for custom hosts (these work without macOS or Desktop):

```bash
lf desktop view show 'wave:product'       # resolve a Dashboard query for your own host
lf --task LOO-12 open --link --json       # target, fallback command, and route
lf desktop events --json                  # stream Desktop window and pane changes
lf help desktop --all --json              # full schema of every Desktop command
```

Programs running in a Desktop shell can also emit OSC 7501 (Rex's Program
Status protocol) to mark a pane available when waiting for input.

## Reference

| `lf desktop` exit | Meaning |
|---|---|
| 0 | Done |
| 1 | Failed, or Desktop API version mismatch (update Desktop) |
| 2 | Bad arguments or query |
| 3 | Target missing or ambiguous; nothing changed |
| 4 | Desktop unreachable (not running or not macOS) |

Run `lf desktop actions` for every verb and shortcut. Full flags:
[`lf open`](lf-reference.md#lf-open), [`lf desktop`](lf-reference.md#lf-desktop).

Next: [Telemetry](telemetry.md): cost, context budgets, transcripts, and
health checks.
