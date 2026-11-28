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

The Desktop presentation command opens a source file or selects lines for review
(for review pages, use [`lf page open`](#review-pages)):

```bash
lf desktop files src/parser.rs
lf desktop files src/parser.rs:24-31 --mode diff
```

A path opens in **Read** by default. Desktop keeps the prior reading position
in Files’ jump history and keeps drafts, Undo and terminal focus. During
active editing or Diff review in a visible pane, the path appears in **Open & Scratch** with a
dismissible `Presented <path> · Open` bar. Dismissing it keeps the file in the
rail. Presenting from a zoomed conversation reveals the file beside it and
keeps keyboard focus in the terminal. An unattached Desktop returns exit status 4.

`lf session rename SESSION 'Resume repair'` updates the
terminal title via standard OSC sequences, plus native cmux tabs/workspaces and
tmux windows. Rex and Herdr users can configure title display in those apps'
own settings.

## Review pages

Review skills (`ship-review`, `explore`, `concept-review`, `qa`, `demo`) compile
a self-contained HTML page under `scratch/` that you can annotate and answer
from any host:

```bash
lf page open scratch/ship-review.html                  # opens in your browser or Desktop Files
lf page open scratch/ship-review.html --print          # print the local URL without opening a window
lf page open scratch/ship-review.html --host browser   # force browser (or --host desktop)
```

| Where you work | `lf page open` does | `⌘↩` sends |
|---|---|---|
| Browser beside Rex or any terminal | Opens `http://127.0.0.1:<port>/<token>/f/<path>` and prints it | `task.steer` (Task pages) or `session.steer` (Wave, standalone Session, repo) through `lf server`; the dock prints the server's delivery line |
| Loopflow Desktop | Opens the page in **Files** quietly (`lf desktop files`) | Notes, choices, and answers join **Diff** line comments in Desktop's shared comments strip and insert into the conversation input |
| Headless Graph step (`lf -b` / auto) | Prints the local URL and exits without popping a browser window | Desktop's `scratch/` watcher shows a quiet **Updated** notice if the Task is open; open the URL or file when ready |
| Offline (`file://` or stopped server) | Double-click the file | Copies the same one-line message to the clipboard |

On the page:

- **Highlight to comment.** Drag across any text (including across inline
  `<code>` spans) to open a floating pill (**Comment (c)** · **Flag** ·
  **Copy**), or press `c` while selected. Saved notes appear as warm-underlined
  highlights (hover to **Edit** or **Delete**), and the top-right **Comments**
  button opens a slide-over sidebar listing all notes in document order with
  click-to-jump, edit, and delete.
- **Diff and file line notes.** Hover any `<lf-diff>` or `<lf-file>` row and
  click `+` (or the line number) to comment on that line; `Shift+click` a
  second line to anchor a multi-line range (`<path>:<start>–<end>`). Line notes
  highlight their rows and join the **Comments** sidebar.
- **Choices and questions.** Click single-select choices (`Fix here`,
  `File Task`, `Drop`), multi-select options, or type into numbered question
  fields. Every selection queues into one reply.
- **Send once (`⌘↩`).** All queued notes, line ranges, choices, and answers
  format onto one line identically across hosts:
  ``(1) `scratch/ship-review.html` § <section> › "<quote>" <words> (2) …``
  In a browser, the bottom composer dock shows each queued item as a removable
  chip (click to jump, `×` to remove), an optional overall direction input, a
  toggleable steer preview, and the server's delivery sentence, clearing the
  queue only on success (even when a headless Graph worker built the page,
  `task.steer` reaches the Task's active work or primary interactive Session).
  In Desktop, the browser dock stays hidden so Desktop's comments strip and
  conversation input remain the single queue; drafts survive switching to code
  files (including for [Tasks on another Machine](#a-task-on-another-machine))
  and clear when sent. Side-by-side HTML diffs in **Diff** stay read-only.
- **Direct action buttons.** A button wired to a Loopflow operation (for
  example, **File Task now**) is separate from the reply queue. Clicking it
  shows the operation kind and fields inline in the browser or in a native
  confirmation sheet in Desktop. Confirm to run it and replace the button
  with an in-place receipt
  (`Filed LOO-1200`). It never enters the browser dock or Desktop's comments
  strip; if you also send queued notes with `⌘↩`, a short
  `[already ran: task.create → LOO-1200]` suffix rides along so the agent does
  not repeat it. Offline, clicking the button adds `Run: …` to the reply for
  clipboard copy.

Drafts are keyed by file path and build hash: reloading the page keeps them,
and rebuilding the page starts clean. If `lf server` stops or restarts while a
page is open in a browser (or a stale URL fails to reconnect), the page shows a
banner (`Loopflow server disconnected — run lf page open <path>`) and `⌘↩`
falls back to copying. Browser loopback serves checkouts
on this Machine; for peer-owned Tasks, open the page in Desktop. Hand-written
`scratch/*.html` files without the page kit keep opening as plain pages with no
dock. Passing a raw `*.page.html` source to `lf page open` exits with the
`lf page build` command instead of serving uncompiled tags.

To author or customize review pages and components, see
[Build and customize review pages](orchestration.md#build-and-customize-review-pages)
or run `lf help page`.

## Loopflow Desktop

The native macOS app reads the same local store as `lf`; it keeps no copy of its own:

- **Sidebar (⌘1–9):** your [Working Tasks](#working-tasks-in-the-sidebar) in
  shared order (dragging reorders them for Wave views and CLI lists too).
- **Paused Tasks:** opening a paused Task on Mac or iPhone opens its conversation.
  Tap ▶ to resume the interrupted step. At review, at a landing stop with Auto off,
  or at the end, ▶ lifts the hold and keeps the Task there. At **Waiting: land**,
  turn on Auto to land or choose a step from **Start from**.
  The chip beside ▶ shows **Pausing…** or **Unpausing…** while the Machine answers.
  `lf task unpause` without `--run` lifts the hold and starts nothing.
  The Auto switch can be turned off during a hold.
- **Task workspace:** three resizable panes: `session` (conversation), `shell`
  (at the Task's worktree), and `browser` (**Brief**, **Diff**, **Files**, Graph
  log, local dev URLs). **Files** opens files read-only by default with one-key
  **Edit** (⌃⌘E), checkout-wide **⌘P** fuzzy find and **⇧⌘F** grep, **⌘F**
  in-file find, **⌘-click** / `gd` jump to definition with **⌘[** / **⌘]** jump
  history, line and review-page comments shared with **Diff** (⌘↩), and quiet
  update notices for `scratch/` writes.
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
- **⌘,:** [Settings](#settings) for appearance, shortcuts, phone access,
  Machines, and scoped configuration files.

### A Task on another Machine

Open a Task that runs on mini, a pool Machine or a rental, and its workspace
is mini's: **Files** browses and saves mini's working copy, **Changes** and
**Diff** show mini's uncommitted work, **Shell** starts in mini's checkout, and
**Session** attaches to the conversation running there. The workspace header
and each terminal name the Machine. Desktop asks your local Loopflow server,
which reaches mini over SSH; Desktop itself runs no `ssh`. A checkout of the
Task left on your Mac from before it moved is never shown in its place.

| When | Desktop shows |
|---|---|
| The Task has no checkout yet | **Not started** in Files, Changes and Shell |
| mini doesn't answer | An error naming mini, with **Retry**; never an empty diff or a shell on your Mac |
| The Task moves to code-1 | The header names code-1 and Files and Changes reload from it. Shells and Sessions already open stay on mini, labelled, until they end; **New shell** starts one on code-1 |
| You have unsaved edits when it moves | Save writes them to code-1 if the file there is unchanged since you opened it; otherwise Desktop shows both versions and keeps your edit |

Save and open Dashboards from the CLI too:

```bash
lf desktop view 'state:available'
lf desktop dashboard query attention 'state:available or state:blocked'
```

Queries support `wave:`, `task:`, `kind:task|wave`, and
`state:open|available|busy|blocked|error|unstarted|idle|active|moving|paused|backlog|working|done|auto`
(`auto`: an open Task with auto on), combined with whitespace (AND), `or`, and parentheses.

### Links in terminal panes

Hold **⌘** and hover over a link in a Session or Shell pane — a Markdown link
from an agent, `ls --hyperlink` or `rg` output, or `lf open --task LOO-12 --link`.
The cursor turns into a hand and the bottom corner shows the target URL or path.
For a malformed or unsupported target, the corner shows why it was refused.
This also works when the pane strip is hidden and while the terminal keeps its
screen during a reconnect or after its relay ends. **⌘-click** opens it:

| Link | Opens in |
|---|---|
| `https://…`, `http://…`, `mailto:…` | Default browser or mail app |
| `loopflow://<repo>`, `…/task/<ID>`, `…/wave/<name>` | The repository's Desktop window, with no alert |
| File inside the Task's active checkout (e.g. `file:///…/scripts/test.py#L42`) | **Files** at that line in Read mode, on this Mac or a remote Machine |
| Local folder, or a local document with no matching Files pane | Default app on this Mac |
| Other URL schemes (e.g. `slack://…`) | A confirmation sheet names the target before opening it |

Repository and Wave Sessions, and local shells left behind after a Task moves,
open local documents and folders in the default app. Remote files open in
**Files** when the pane shares the Task's current checkout. The file URL can
carry the remote hostname.

Clicking a checkout file link in a Dashboard tile enters the Task and opens
**Files** beside its Session.

Desktop resolves `loopflow://` links as Work addresses. File links need an
existing absolute path without parent-directory segments and can carry a line
fragment in the form `#L<n>`. Outside Files, Desktop refuses to launch app
bundles, scripts and executables. Desktop also refuses remote folders, files
outside the active remote checkout, malformed links, and links with control
or bidirectional text characters. Failed clicks say why in the bottom-corner
notice.

Every held terminal identifies itself as `TERM_PROGRAM=ghostty` (dropping any
inherited `TERM_PROGRAM_VERSION`), so agents and CLIs emit clickable hyperlinks
whether a Session started from Desktop, a Graph worker, or another terminal.
Sessions already running keep their launch environment until their next start.

### Changes show at once

Actions in Desktop (Complete, Abandon, Reopen, Claim, Unassign, Pause, Unpause, Move, Rename)
update the UI on the click, including for remote Machines and the phone. If the
Machine refuses, the row returns and a single bottom Notice explains why
(auto-dismissing after 6 seconds; **Complete** posts a Notice with **Undo**,
which reopens the Task).

Completing a parent with open children asks before writing. In Desktop,
**Complete Task…** opens a dialog naming the open children: when any open
descendant would be abandoned, **Complete & Move Children to \<ancestor\>** (or
**Complete & Move Children to Top Level**) is the default action and
**Complete & Abandon Children** is destructive; when every open descendant
already has a merged PR, **Complete & Finish Children** is the default with
**Complete & Move Children to …** beside it. On iOS, swiping a parent left and
tapping **Complete** keeps the row in place and asks between **Move Children
Up** and **Abandon Children**. **Reopen Task** works on both done and abandoned
Tasks.

If the Machine pauses briefly, edits still show immediately and flush once it
reconnects. After ~10 seconds offline, mutation controls disable until the
connection returns. See [Connect clients](server.md) for connection recovery.

### Working Tasks in the sidebar

Mac and iPhone show three groups within Working. **Active** holds the ordinary
working rows. Turning auto off moves a Shipping Task to Active immediately
if its Graph still has a landing ahead. It returns to Shipping after landing
when only automatic steps remain.
**Shipping** holds Tasks with auto on and Tasks on a landing Graph with no human
step left before landing. Graphs such as `research` stay Active unless auto is on.
**Paused** holds owned Tasks whose Graph is on hold, including those with auto on.
Their rows still show the conversation’s status and fall back to Paused when
it has none. Paused Tasks stay out of Portfolio Now. Each group keeps the
shared Working order.

In the default order, open the second Working Task and send a prompt. Its row
stays second as the conversation turns Busy and then Available again.

Drag a row in the repository sidebar or the Working Tasks grid to save its
position on this Machine. Wave views, `lf task list`, `lf wave show`, and the
phone feed follow the same relative order. Working Tasks start newest first,
with canonical Task ID breaking ties. New arrivals appear above rows you’ve
arranged. Sidebar and phone rows show the current Graph step beside the status
when the Machine reports it.

Turn on **Sort by status** in a repository or Wave sidebar, or from **⌘K**,
to sort within each Working group by Blocked, Error, Available, Idle and Busy. A Task with both
busy and available conversations sorts as Busy. Each group follows the shared
Working order.
Sending a prompt updates the open Task’s status, but its row stays in the
group it occupied when you opened it or turned sorting on. Other rows can
move above or below it. Leaving the Task or changing the filter releases
its group. Turn sorting off to restore the shared order and drag rows in
the repository sidebar. The Working Tasks grid uses the shared order, and
saved Dashboards use their own arrangement.

A drop shows immediately. If another window has already reordered the list,
the Task still lands on the side of the target you chose. A refused drop
restores the last confirmed order in both lists and shows a Notice.

In **⌘K → Sidebar shows**, choose **Working tasks**, **Available**, or **Busy**.
Filters use current activity. A Task with both available and busy
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

**Objective ›** expands the Wave's objective and shows its Project's name,
status and tracker sync, as on the Portfolio page.

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
  aren't undone; an abandoned Task keeps its ID and history and can be reopened.

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

☺ explore   plan   build   ☺ ship-review   deliver
You take part at explore and ship-review.

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
[▶]   [Run: explore ▾]   plan   build   ☺ ship-review   deliver       Auto ◯   ✓

Not started

It will run feature, Product's default.
You take part at explore and ship-review.

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
✓ feature                 Explore, build and deliver                 2 stops
  code                    Build and deliver                          1 stop
  redesign                Rethink a started Task                     1 stop
  research                Investigate and report, with no PR         1 stop
  More ›
  Run without stopping
  deep-clean              Delete what an area no longer needs        never stops
  incident                Restore first, then fix the cause          never stops
  optimize                Move one number                            never stops
  quick                   Build a small, clear change and land it    merges without asking
  task-design             Draft the Task design                      never stops
```

Several Tasks whose workflows differ show the split,
`Start 3: feature 2, code 1`, and the menu opens with "Each Task's own
workflow"; clicking Start runs each on its own. Pick a workflow below it to run
all of them on that one. Workflows under "Run without stopping" never wait for
you; one that lands the PR before any stop, like `quick`, reads **merges without
asking** (`lf task auto off` still holds before `land`). **More ›** holds the pieces other
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

### Settings

Press **⌘,** to open Settings. Search for a pane, label, configuration key,
alias or description. Select a result with ↑/↓ and Return to reveal the
control. A switch in the results edits the destination shown beside it.

**General**, **Appearance**, **Shortcuts**, **Agents**, **Machines**, and
**Mobile** hold personal preferences. Use **Machines** to check connections
and set the plan hub. **Open ~/.lf/config.yaml** opens the global file for
settings such as Machine pools and SSH forwarding.

**Project & Wave Config** holds agent overrides, Context, Git & Release, and
Machine placement. It opens on the active checkout or the first repository
in your portfolio. Use **Choose Checkout…** to select another directory.
**Applies to** selects a checkout or Wave. **Save in** selects your file or
the shared file. Saving the first setting creates the file. **Open file…**
opens the current settings document in its associated app. If shared Wave
settings come from `GOAL.md`, Open shows the goal and **First change saves to**
shows the path to `config.yaml`. Once an edit creates the YAML file, Open shows it.
A dot beside the pane means the selected file declares a setting. Switching to
a personal pane keeps the checkout and destination ready for your return.

Expand **Skill or Graph step** to see launch settings for that target.
**Inspect general work** returns to all settings. A step needs an `id:` before you can save its
preferences. Git & Release applies to the repository. While viewing a Wave,
**Inspect repository settings** returns to its checkout. Opening a search result
selects general work or the checkout when the setting requires it.

**In effect** appears when the selected file declares a setting or another
source supplies a value beyond the built-in default. For example, open
**Project & Wave Config** on a checkout where the shared file sets
`context.diff: true` and your file sets it to `false`. Choose **Shared
repository settings**. The switch shows **On**. **In effect** still shows
**Off · My repository settings**. Expand it to see the candidates and paths.

- **Reset** removes a simple control’s declaration immediately. Clearing a text
  or number field also resets it. YAML editors preview removal with **Reset…**.
  For additional documents, **Save empty list** clears inherited entries. Reset lets them
  apply again.
- **Move to Shared Repository…** appears when the shared slot is empty. From
  a private Wave you can move to your repository settings. Review the YAML
  changes and the projected result before confirming. If either file changes
  the preview, review it again. **Finish interrupted move…** completes a
  partial move. If the declarations have diverged, **Dismiss pending move**
  keeps the current contents of both files.
- Switches and pickers save immediately. Text fields save on Return or focus
  loss. Escape discards the text. YAML editors have **Apply** and **Cancel**.
  **Undo** or **⌘Z** restores a saved edit even after switching destinations.
  If the declaration changed on disk, use **Refresh** to review it before
  retrying. A rejected edit leaves its field editable. An unavailable scope
  keeps the last reading visible and pauses edits. If the local server
  restarts, Settings reconnects automatically and enables edits when fresh
  values arrive.
  Settings and the CLI share the same [configuration files](config.md).

**Appearance** selects themes or opens the color editor. **Shortcuts** shows
the keyboard reference. **Mobile** shows a green dot and pairing code when
[phone access](server.md#phone-access) is ready. While **Machines** is open,
it checks up to four registered identities at once and distinguishes
connection errors from ended rentals.

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
[`lf open`](lf-reference.md#lf-open), [`lf page`](lf-reference.md#lf-page),
[`lf desktop`](lf-reference.md#lf-desktop).

Next: [Telemetry](telemetry.md): cost, context budgets, transcripts, and
health checks.
