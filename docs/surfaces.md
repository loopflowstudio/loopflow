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

- **Sidebar (⌘1–9):** your [Working Tasks](#working-tasks-in-the-sidebar) in
  shared order (dragging reorders them for Wave views and CLI lists too).
- **Task workspace:** three resizable panes: `session` (conversation), `shell`
  (at the Task's worktree), and `browser` (Files, Graph log, local dev URLs).
- **Dashboards:** saved side-by-side grids of Tasks selected by query. Each
  tile is headed by the Task's sidebar row, with the sentence saying where its
  workflow stands.
- **One row for a Task:** the sidebar, a Dashboard tile, the title bar, the
  Task's own page and the Portfolio show the same status word for a Task, and
  the same line when it needs someone. The Portfolio's Now list holds the
  Tasks that need someone; one that waits only to be started is in the
  backlog. Click the row anywhere and the Task opens.
- **Wave and repository views:** the scope's conversation beside its
  [backlog](#the-backlog) and Task tiles below; a Task with auto on carries an
  Auto badge on its tile.
  Every pane zooms and hides on hover; the header's `Show…` menu restores
  hidden parts.
- **⌘K:** jump to any Wave, Task, Session, or Graph.

Save and open Dashboards from the CLI too:

```bash
lf desktop view 'state:available'
lf desktop dashboard query attention 'state:available or state:blocked'
```

Queries support `wave:`, `task:`, `kind:task|wave`, and
`state:open|available|busy|blocked|error|unstarted|idle|active|moving|backlog|working|done|auto`
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

### Working Tasks in the sidebar

Open the second Working Task and send a prompt. Its row stays second as the
conversation turns Busy and then Available again.

Drag a row in the repository sidebar or the Working Tasks grid to save its
position on this Machine. Wave views, `lf task list`, `lf wave show`, and the
phone feed follow the same relative order. Working Tasks start newest first,
with canonical Task ID breaking ties. New arrivals appear above rows you’ve
arranged. Sidebar and phone rows show the current Graph step beside the status
while a Task is at a step.

Choose **Sort by status** in the repository sidebar or from **⌘K**, or run
`lf desktop sort`. This puts Blocked Tasks first, followed by Error, Available,
Idle and Busy. A Task with both busy and available conversations sorts as Busy.
Each group starts with its newest Task. The order stays put as
conversations change status.

A drop shows immediately. If another window has already reordered the list,
the Task still lands on the side of the target you chose. A refused drop
restores the last confirmed order in both lists. A Notice explains a refused
drop or sort, even if the next read fails.
Sort takes its order from the Machine. A drag made after Sort still shows
immediately and saves after the sort.

In **⌘K → Sidebar shows**, choose **Working tasks**, **Available**, or **Busy**.
The filter keeps the list's order. A Task with both available and busy
conversations appears in both filters. **⌘1–9** and **⌃Tab / ⌃⇧Tab** follow
the visible rows. The open conversation stays selected if its row leaves the
filter.

### The backlog

Open a Wave and its open work nobody is running is one ranked list, the next
work first. Click a row and its brief opens under it; nothing starts until you
press Start.

```text
Product
demo-20260924  Objective ›                                               New
[All 46] Mine 1  Available 43                                        Search…
1 selected  [Start with feature ▾]  Claim  Edit  Move ▾  Abandon…  6 of 46 ↑ ↓
☑ ⌄ LOO-452  Customize terminal chords                 claimed by you      ···
             Let people rebind chords per repository and drive them from…
             Show all 412 words
☐ › LOO-456  Keep briefs readable on narrow windows                        ···
☐ › LOO-753  Retry provider launches        Infrastructure, claimed by Ana ···
☐ › LOO-781  Trim the launch prompt         Idle, at demo, PR #1902        ···
```

A claimed row says whose it is: "claimed by you" or "claimed by Ana". An
unclaimed row says nothing; the **Available** count already does. A row shows
its Wave when it isn't the one you're looking at. Most rows have never run. A
Task someone started and nobody holds now is here too, and says where it
stands: the status word its sidebar row would show, then its step and its PR.
A Task that is blocked or waiting adds a line with the reason. The backlog
doesn't run a started Task: its button reads **Open**.

**Objective ›** opens the Wave's Project: its objective and key results, drawn
as they are on the Wave's Portfolio page.

- **Find.** Search matches titles and briefs. **Mine** is what you've claimed;
  **Available** is everything not claimed by someone else. Changing the filter
  or the search deselects the rows it hides, so an action reaches only rows
  you can see.
- **Select.** Click, the checkbox, ↑↓, ⌘-click, ⇧-click and ⇧↑↓ select. One
  selected Task shows its brief, cut at nine lines past 900 characters, and
  its chevron points down; click the chevron to close it. Several selected
  show none. ⌘A selects all and Esc clears.
- **Act.** The toolbar says what it acts on, "3 selected" or "Nothing
  selected", then acts on it. `···` at a row's end and right-click open the same actions: Open, Start, Claim,
  Edit brief, Move to another Wave or to the top, Move up/down, Copy ID and
  Abandon…. Abandon lists the Tasks it will remove and names the ones with a
  PR before it writes. The narrowest window drops the position and ↑↓ first.
- **Order.** Drag rows, press ⌘↑↓, or use Move ▾ › To top. The order is the
  Task's rank in its Wave, the same one `lf task list --wave product` prints and
  Linear shows. Over a whole repository, order belongs to each Wave, so
  reordering is off there.
- **Claim without starting.** Claim makes a Task yours so nobody else takes it;
  it runs nothing. Claiming a started Task picks it up where it stopped: it
  leaves the backlog for Working.
- **Undo.** ⌘Z undoes a reorder, a claim or a brief edit. If someone changed
  the brief since your edit, undo leaves it and says so. Start and Abandon
  aren't undone; an abandoned Task keeps its ID and history.

| Key | Does |
|---|---|
| ⌘↩ | Start, or open a started Task |
| C | Claim |
| E | Edit the brief |
| ⌘⌫ | Abandon… |
| ⌘↑ ⌘↓ | Move the selection up or down |

#### New Task

Press ⌘N anywhere, or **New** in a backlog, and say what you want:

```text
What do you want?

  Let people rebind chords per repository

In Product ▾, running feature ▾, Product's default

☺ explore   plan   build   ☺ ship-review   deliver   ☺ verify
You take part at explore, ship-review and verify.

                                        Cancel   File   Start ⌘↩
```

The sheet opens on the Wave you were looking at. The first menu moves the Task
to another Wave or to the repository; the second picks its workflow, from
[the same list Start offers](#start-picks-a-workflow). After the comma is whose
choice the workflow is: the default of the Wave or repository named before it,
your own default, or "for this Task only" once you pick another. Pick the default again to go back.

Under the sentence is the workflow as the Task will show it later: its steps,
a person before each one where you take part, then the same thing in words. A
workflow with no such step reads "Runs on its own." Change the menu and both
lines change with it.

- **Start** (⌘↩) files the Task and runs it.
- **File** runs `lf task create` without `--run`: the Task lands in that
  Wave's backlog, selected, and nothing starts.
- **Cancel** (Esc) closes the sheet.

If the Task can't be filed, the sheet stays open with what you typed, "Not
filed." and the Machine's reason. If it is filed and its run doesn't start, the
request stays on screen with "Not started.", the error, **Try again** and
**Discard**.

#### A Task that has not started

Open a filed Task and it says what Start will do, in the sheet's words:

```text
feature   ☺ explore   plan   build   ☺ ship-review   deliver   ☺ verify   Start

Not started

It will run feature, Product's default.
You take part at explore, ship-review and verify.

[Start]   Start with… ▾
```

**Start** runs that workflow. **Start with…** lists the others; picking one
starts the Task on it. When the Machine would refuse the run, Start is off and
its tooltip says why. The brief beside it names the Task. In a Dashboard tile
it is one line: "Not started" and **Start**.

#### Start picks a workflow

Start runs a Task's Graph, which Desktop calls its workflow. The button names
the one it will run: `Start with feature`, the Task's own, else its Wave's, else
the repository's. ▾ lists the others, each with its description and how many
times it stops for you:

```text
✓ feature                 Explore, build, deliver and verify      3 stops
  code                    Build, deliver and verify               2 stops
  redesign                Rethink a started Task                  1 stop
  research                Investigate and report, with no PR      1 stop
  More ›
  Run without stopping
  deep-clean              Delete what an area no longer needs     never stops
  incident                Restore first, then fix the cause       never stops
  optimize                Move one number                         never stops
  queue                   Compress, integrate upstream changes    never stops
  task-design             Draft the Task design                   never stops
```

Several Tasks whose workflows differ show the split,
`Start 3: feature 2, code 1`, and the menu opens with "Each Task's own
workflow"; clicking Start runs each on its own. Pick a workflow below it to run
all of them on that one. Workflows under "Run without stopping" never wait for
you. **More ›** holds the pieces other
Graphs include, such as `build` and `deliver`, and Graphs with no
`description:` ([describe a Graph](orchestration.md#describe-a-graph)). A
Graph that runs for a Wave is not offered for a Task. The
[New Task](#new-task) sheet's workflow menu and an unstarted Task's
**Start with…** are this list.

Start reaches unstarted Tasks only. Select a started Task and the button reads
`Open`, with no ▾: it goes to the Task, where its step, its PR and what running
it would do are shown. A selection of both counts the unstarted,
`Start 2 with feature`, and the tooltip says "1 started Task: open it to
continue". Several started Tasks and nothing else leave Start off. When the
Machine would refuse every unstarted Task selected, Start is off and its
tooltip says why.

Starting several starts each one in the click. A Task the Machine refuses comes
back with its own Notice, and the others stay started. Started Tasks leave the
backlog and appear in the sidebar.

#### When the list is empty

Every empty list says why in one line and offers one step beside it: **Mine**
with nothing claimed offers **Show available**, a search with no match offers
**Clear search**, and an empty Wave or repository reads "Nothing is waiting in
this repository." beside **New**. When a read fails, the last rows you saw stay and
a Notice says so. A list too large to load at once says
`Partial: searching the 200 loaded`.

### When nothing is there, or something failed

A pane with nothing to show says what happened in a title, adds one sentence,
and puts the next step on a button:

| You see | Next step |
|---|---|
| **No planned Work in this repository.** Say what you want built, or talk it through first. | **New Task ⌘N**, **Open the repository conversation** |
| **Conversation not open.** | **Open conversation** |
| **Choose Work.** Pick a Task or a Wave in the sidebar, or press ⌘K to find one in any repository. | |
| **Selected Work is unavailable.** The latest plan does not include it. Its workspace and conversations are kept. | **Read the plan again** |
| **Nothing needs action.** No live or stopped work across these Waves. | **Show roadmap** |
| **This dashboard is no longer available.** | **Show working Tasks** |
| **No activity in this window.** Work creation, runs, PRs and steers appear here. | |

A repository with no Waves, no Tasks and no conversation opens on the first
row alone. **Open the repository conversation** starts the conversation and
brings the repository's page: the terminal, the backlog and its Tasks.

A failure names what failed, shows the Machine's message as it came, and keeps
**Retry**:

```text
This repository did not open

fatal: not a git repository (or any of the parent directories): .git

[Retry]
```

The message is selectable, so it can be pasted into a conversation. A state gets a button only for something Desktop can do from
there; one with no next step has none. In a dashboard tile the same state is
one line: its title and its button.

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
lf desktop capture -p backlog                     # the Wave's backlog as text: counts, then rows in order, each with who claimed it
lf desktop zoom --on -p backlog                   # zoom a pane; --off restores
```

Without `-p`, commands act on the terminal they ran in. `-p` targets a pane
(`session`, `shell`, `browser`), Wave view part (`backlog`, `tiles`), tab
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
A live interactive Session that reports a finished turn is Available once
its last working report clears. Typing dismisses finished and error reports.

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
