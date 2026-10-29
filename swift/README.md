# Loopflow for macOS

```bash
uv run python scripts/loopflow-dev.py run          # build and launch
uv run python scripts/loopflow-dev.py install      # install without launching
uv run python scripts/loopflow-dev.py run-debug    # launch with logs visible
uv run python scripts/loopflow-dev.py test         # run Swift tests
```

The development app's main-Machine commands forward to the installed CLI.
Select a private `LF_HOME` to use this checkout's bundled CLI with separate data.
For populated data, use [offline data clones](../docs/data-clones.md):
`lf clone run` selects the clone's pinned helper, confines Desktop and its embedded
terminals, preserves source checkouts read-only, and reports copied Sessions'
continuation limits. A bare SQLite copy provides no filesystem, provider or client
isolation.
Repository selectors list only Git main checkouts. Linked worktrees stay visible
only through the Task Work that owns them.

A returning launch opens on the workspace saved at `<Machine>/desktop-cache/workspace.json`
and shows **Updating…** until this launch's reads replace it. Saved rows open
and navigate; Graph process state, Task condition, Session state and every action except
opening a Session wait for the fresh read. The Portfolio window opens from the
same saved workspace. A failed refresh keeps what is shown under one
**Couldn't update** line. Repeated identical Session errors leave the displayed
reading unchanged; failed per-Wave or truncated replacements keep previously readable planning,
including its saved copy. A new error or recovery updates it.
The bundled helper cannot migrate the installed store: schema changes require a
published release installed through `lf install`. Delete the file to start
from **Loading workspace…**.

```sh
uv run python scripts/benchmarks/desktop-performance/timings.py   # how long your launches took
```

Each launch records when its first frame, saved workspace and fresh workspace
arrived, and how long each `lf` read took, under `desktop-cache/timings/` in
the Machine. Durations and command names only; the files stay on this machine.

Working Tasks lists every started, uncompleted Task, on any Machine this store
records, in the repository plan's order (waiting first, then rank). It is the
repository's built-in query, so opening a Task neither adds nor reorders it.
Click a Wave to scope the sidebar to its Working Tasks and subwaves; its board's
Backlog keeps the unstarted ones. **‹ Repo** returns to Working Tasks. Unstarted Tasks
open their brief beside **Start**, which runs the Task's Workflow; opening alone
starts nothing. Waves follow Tasks in the same scrolling sidebar, with counts
from the planning inventory.

Click a row to select its retained workspace. The title-bar **Repo › Wave › Task**
path follows hierarchy: click a name to go there, or the **›** after it to list and
search what it contains; Back and **⌘[ / ⌘]** follow visits. **⌘K** searches Work
across repositories and opens another repository in its own window. Its
**Sidebar shows** picker filters Working tasks / Available / Running without
changing the selected Work. Click a Wave and its name moves into the
sidebar header; the rows below are its Tasks, in plan order, until
**‹ repo** returns to Working Tasks. **⌘1–9** selects the sidebar's Task rows by
position and **⌃Tab / ⌃⇧Tab** cycles them. Missing slots do nothing. Selecting a
Dashboard's Task, from any of these, keeps the Dashboard in the path, as its tile does.
With sidebar focus, **⌘F** searches its visible Tasks and Waves; **Search all Work**
widens the same navigator. **Shift-F10** opens the focused Task's actions without
changing the open workspace.

The Task action menu moves work into a Wave or back to **No Wave** through the shared planning operation, retaining its workspace and drafts.

Each Work keeps one pane layout and document store. Repository and Wave
entry shows their retained conversation or an explicit **Open conversation** action.
Waves pair that conversation with their objective, status, current Project and
full Task inventory, followed by their metrics. **Project history**, **Realign Projects…** and **Workflow**
open the existing planning inspectors. The sidebar starts at 240 points wide;
drag its edge to resize or use the title-bar button or **⌘\** to toggle it.
Its scroll position survives Wave and Task navigation. Working order survives
relaunch; ordinary launch still opens the repository, without acquiring a Session.

Closing a repository window keeps its panes, selected Work and file drafts until
Desktop quits. Reopening a known repository shows retained content immediately,
even while repository resolution waits or fails. Errors remain visible; new Task
links wait for resolution before delivery. Unknown repositories need an initial lookup.

Selecting a retained Session reuses its surface. When a surface needs reconnecting,
Desktop attaches to the recorded execution without starting a provider, replaying
input or taking control from another viewer. A failed connection stays visible.
**Resume** does what `lf resume` does: it attaches to a running conversation, or
otherwise reopens its saved provider history. At launch Desktop runs
`lf resume --all`, so conversations a reboot or crash interrupted come back
unasked. Repository, Wave and Task conversation entry share metadata-only
preparation; only a newly admitted conversation offers its first launch.
Finite CLI requests time out after 30 seconds. Canceling preparation ends only
that request; running Sessions continue. If a change times out, refresh to learn
its result before retrying.

Pane headers name the known provider, conversation and directory; arbitrary terminal
titles remain context under **Shell**, without guessing which program is running.

Task rows show reported program status, Task IDs, stored PR links and the recorded
remote Machine. Task IDs link to the issue when its URL is recorded; otherwise they
remain plain text. Qualified Session excerpts show their source age; stale output
falls back to a labeled summary. Local Machine names stay hidden. Links open
separately from the Task row; unknown activity and live clients without reports
show no activity label.

Work selection uses the theme’s selection fill and contrasting text.
File selection keeps a neutral fill and edge; keyboard focus has a separate outline.
Ordinary controls and links use text roles; the theme’s accent emphasizes deliberate actions.
Status labels and shapes retain their meaning across palettes.
Choose **Settings → Appearance** to preview the whole workspace. Set
**Light theme** and **Dark theme** independently; **System** follows macOS using
both. System lists both pickers; Light or Dark lists only its own. Each picker
shows built-ins, then **Yours**, ending with a dashed **Import file…** tile.

```yaml
# ~/.lf/config.yaml
theme: "light:Linen,dark:Hearth"   # System
theme: "Hearth"                    # Dark (or Light) mode: one name
```

**Apply** rewrites only that line; **Cancel**, Escape or closing writes nothing.
Edit it by hand and every window follows; a misspelled name keeps the last theme
and names the fix. Absent means Loopflow Light/Dark. A repository's
`.lf/config.yaml` never sets the theme.

Built-ins are 12 light and 12 dark. **Loopflow Light** keeps the original
cream/burgundy; **Loopflow Dark** is slate with warm bronze; **Linen** and **Hearth**
are quieter light and charcoal alternatives.

Your themes are Ghostty theme files in `~/.lf/themes/`; `~/.config/ghostty/themes/`
is read too, never written. Names resolve to yours, then built-ins, then Ghostty's.
A file named like a built-in overrides it. **Customize** on a built-in writes your
copy; on yours it edits in place. **Delete** removes yours (**Revert to built-in**
for an override). **Import file…** copies a theme into Yours, accepting only the
picker's appearance; a dark file in the Light picker explains itself instead.

```
background = #171924
foreground = #cad2ef
palette = 4=#7aa2f7
loopflow-accent = #3d59a1    # optional app roles; the rest derive from ANSI colors
```

Files read up to 256 KiB of UTF-8. Six-digit RGB background/foreground are
required; cursor, selection, `palette = N=#RRGGBB` and `loopflow-` roles are
optional. Preview explains defaults, repeats and omitted settings. Fonts,
commands, keybindings, includes and theme references are never followed.

Unreadable app/default/selection/cursor text prevents Apply; ANSI colors keep
their source values. Personal non-color Ghostty settings still load; its `theme =` is ignored and its file never written.
Source and license notices ship offline with the catalogue.

**Auto-Ship** hands a Task off; it leaves Working tasks and its ship process lands it.
It sits at the right end of the Workflow bar under the jump bar, beside the Task's
stages as dots (click them for each stage) and what is running now. While shipping,
the same spot holds Stop Auto-Ship, and the toast's Undo is Stop Auto-Ship. Open a Dashboard with `state:shipping` to watch it.

```text
⌘T          new shell tab in the Task’s Shell tool
⌃`          show / hide Shell
⇧⌘E         show / hide Files
⌘⌥←/→/↑/↓   focus the visual neighbor
⌘⇧Return    zoom the focused pane
⌘W / ⌘⇧T    close the selected tab / undo close
⌘F          find in the focused terminal
```

Panes sit in rounded islands with small gutters; drag a gutter to resize.
**View → Zoom Focused Pane** and **View → Focus Pane** expose the same pane controls.

The focused pane has a quiet raised edge. Copying selected text or a command
block briefly confirms the number of copied lines without moving focus.
Find reserves its own row above the terminal, so it never covers a match.
Return / Shift-Return select the next / previous match; Escape closes Find.
Search and terminal state stay with their retained pane when switching Work.

Task workspaces start with the Session on the left and Files on the right,
at 61.8/38.2. Use **View → Toggle Shell** or **Toggle Files**, or their shortcuts.
If every pane is hidden, **Show pane** restores the retained pane; **Open conversation**
remains available when the Task has no conversation open.
Shell holds retained shell tabs; Files holds documents, **Brief**, **Changes** and
recorded **Pull request** tabs. Hidden tools keep their clients and drafts. Files starts with its chooser closed. Click the file-browser toggle or press
Command-P in the file pane to search **scratch** and files changed from the
branch's recorded parent commit. Selection closes the dropdown; pin it to keep
the list alongside the document (Control-Command-B). Escape dismisses the dropdown.
Sidebar selection switches the whole
Task workspace; selections inside a Task can focus its companion.
Resizing or closing the browser is remembered while the window stays open;
layouts are not yet restored after quitting the app.

Choose **PR** for a compact GitHub page: status, checks, pending reviewers,
description and change totals. Expand **Files changed** and individual files to
read published patches. Refresh updates GitHub data using the signed-in GitHub
CLI on this Mac. Missing access and omitted large/binary patches are shown inline;
local uncommitted edits stay in Files.

Open **•••** or press Command-/ for the selected Task’s actions. Row context
menus use the same actions for the clicked Task. Command-T adds a shell tab. The main Session's menu switches between **Claude**,
**Codex**, and **OpenCode**, continuing the same Session with its history.
**Reset session** starts a fresh primary conversation/process from current Work
instructions, without replaying the previous conversation; its history remains
available. Reset preserves the previous terminal and client. Provider switching
first reads its saved history, then replaces the current provider process. If the
provider has not saved a transcript yet, the current terminal stays open; finish
its first turn and retry the switch. A failed status observer reconnects on the
next status event without replacing the conversation.
Repository, Wave, Task and exact Session
preparation all use `lf resume --prepare`; reset uses `lf session new` for its scope.

Shell panes with Ghostty shell integration group each completed command and its
output into a full-width block. A command that exited non-zero has a red
background. Click anywhere in a block to select the whole unit; drag to select
text instead. There is one selection at a time, and Command-C or the context
menu copies it: a block copies its command and all of its output. Right-click
keeps the selection it lands on. A selected block clears when you type or press
Escape. Command-Up and Command-Down jump between prompts. The live prompt remains ungrouped;
Session/provider panes keep their native TUI behavior and do not expose shell
command blocks.
Automatic integration depends on the configured shell; macOS `/bin/bash` is
excluded by the pinned Ghostty build. A zsh shell still on the macOS default
prompt gets a small dim directory line above each bold command, with the gap
between blocks shared evenly; any other prompt is left as it is.

Terminals start from Desktop's own terminal settings. A launcher's `NO_COLOR`,
`TERM`, pager and agent variables do not reach them, so provider CLIs keep their
colors however Desktop was opened.

If macOS cannot provide Ghostty's display link, terminals use timer rendering.
The app logs the CoreVideo error code; the user's Ghostty configuration is unchanged.

A Task workspace holds three designated panes — session, shell and browser —
arranged freely by dragging, zoom and hide; Wave and repository workspaces hold
only the session pane. The session pane shows one conversation and switches when
another opens. ⌘T adds a shell tab. The browser's tabs are the Task's Files, its
Graph processes and http(s) pages. ⌘W closes the selected tab, or hides the pane.
⇧⌘T puts the last closed tab back where it was, its shell still running.

**View → Toggle Files** shows a Task's checkout in the browser pane beside the
retained terminal. Expand folders to read one page of up to 500 entries;
**Load more** continues the directory and **Show ignored** includes ignored paths.
Tracked, unchanged and untracked files remain available without an active PR.
**Changes** optionally compares the worktree with the Task PR's
recorded base; **HEAD** compares with its current commit. The displayed SHA pins
the file list and selected diff. A recorded PR links above the file navigator.
Use ↑/↓ in the navigator to select files; hover a row for its full path.
Closing the pane keeps its drafts in this window.

**Read** is where documents open: Markdown renders formatted, HTML as a live
page that runs its scripts but never reaches the network. Links open in the
browser. **Edit** retains editable UTF-8 drafts and selection in this window;
**Diff** shows the draft comparison read-only.

When an agent writes a `scratch/**/*.md` or `.html` file, Files reloads it if it
is showing, or follows it. It takes the view only when nothing is selected, when
the current document was itself followed, or when you haven't touched Files for
two minutes. Edit, Diff, an unsaved draft or keyboard focus in Files keep your
view; the page then opens as a tab beside it.

Task headers show the recorded checkout;
terminal and files follow that Task without a separate worktree selector.
Changing Work assignment leaves the recorded checkout Machine and path intact.
The browser retains its opening location through Close/Undo, including a first open
without a Session link. New shell tabs use the Task's recorded location;
existing tabs and file drafts keep their original locations.
Unknown or changed owners stay unavailable without redirecting retained drafts.
Remote paths are never opened or watched on the presentation Mac. Filesystem events
on the execution Machine refresh retained documents, including hidden drafts. Disconnects
retain drafts and show an error; Refresh reconnects. Stale owner readings are discarded.
Switching files, hiding Files and returning to the Task keeps the draft.
Files reached through symlinks remain readable within the checkout and show a
read-only explanation. If a regular file becomes a symlink, its retained draft
and Undo survive while editing, Save and autosave stop.

Typing autosaves after two quiet seconds. Disable **Autosave Task Files** in the
Settings → Editing for explicit **Save** (⌘S). Both modes receive filesystem changes live,
including atomic replacements. Disjoint local edits survive; disk wins overlaps
without a conflict dialog. Incoming updates reset Undo to the surviving local
edits, so Undo cannot restore text the LLM replaced. IME composition completes
before incoming changes apply. Missing or invalid files retain dirty drafts and
cannot be saved. Unsaved drafts do not survive closing the window.

**Open saved versions** opens recovery folders in the checkout's Git metadata,
including files later renamed or deleted. Every completed Save retains its draft,
receipt and displaced inode; no version is created for each keystroke. Ordinary
reads and saves do not scan old recovery history. Inspect late writes explicitly
with `lf task file ISSUE PATH --recoveries --json`. Retention has no automatic
cleanup or size cap; each settled save can retain up to two file-sized versions.
Removing the worktree removes its recovery. Each window holds separate drafts
and receives the other's writes through the same disk-wins policy.

Save preserves file mode and exact UTF-8 bytes, including BOM and line endings.
Revision checks refuse already-observed changes before the editor reconciles
and retries. Arbitrary writers can still race publication; this is not exclusion
of concurrent writes, extended-attribute preservation or power-loss durability.
Symlink paths cannot be saved. A symlink introduced during exchange is retained
without changing its destination.
Binary, missing, unsupported and over-1-MB files have explicit states. Extremely
long lines retain the native editor's known responsiveness limitation.

Select a repository, then a Task to open its conversation. Its workspace retains
conversations, shells, file drafts and layout across visits. Use **+** for supporting
panes; expand **Overview** for Task details. Active clients elsewhere require
explicit **Move here**. An ended Session shows a bounded excerpt of its final
terminal screen; reopening is explicit and preserves the conversation's history.

Exiting a disposable CLI conversation retires it from navigation while keeping
its provider history. Ctrl-C interruption is recorded; interrupted Graph reviews,
Task conversations and repository/Wave primaries remain reachable.
Stopping a response alone never closes the conversation or completes a review.

Each Session or shell's **…** menu holds its pane actions. **Collapse** hides a pane while its process
continues; **Expand** returns to it. **Focus** gives one conversation the content
area; **Restore** brings back the panes and files. **Terminate** ends a shell.

The sidebar lists Work without conversation counts.
Repository and Wave conversations retain their own scopes. Shell-launched unscoped
conversations stay out of the sidebar without deleting their history.
**Available** means the conversation can be joined. Historical feedback remains
readable without granting completion authority. Failed provider starts offer recovery.

Search Work names and identifiers with **⌘K**. The repository name opens its
conversation. Unscoped Sessions remain available through `lf session list --orphan`.
Missing planning or selection readings retain known Work and show the read error.
**Task details** opens the description, comments and Run history.

Opening a Task enters its retained panes. **Open conversation** explicitly reuses
or prepares its primary through `lf`; it never starts the Workflow or takes over
another client. Returning preserves panes, drafts and focus, including historical
conversations. Explicit bare CLI Task links still support inspection without
conversation acquisition.
The compact header shows the current Workflow position and available actions;
**Overview** expands its graph and description above the multiplexer. Opening
feedback stays inside the terminal area. Resolving a conversation installs its
record and pane before ending preparation, including explicit Session links.
Cached switches reuse their panes and drafts without preparing again.
When the selected Task completes or is canceled, it stays visible with a green
**Completed** or red **Canceled** header. Navigating away restores the ordinary
sidebar filters. **Back** (⌘[) returns to the retained workspace and its sidebar
row, preserving conversations, files and drafts without starting execution.
The expanded graph scales to the window:
`start` and `end` as circles, each node a box saying what you do there, one
labelled arrow per edge, a loop back as an arc over its node. The current node
or running edge is marked. The labels of the edges leaving the current node
are buttons; each runs `lf -b task run` as a child of the app. No edge is offered while one runs. An edge whose Graph stopped holds the
Task, is drawn stopped, and is offered again with the others leaving its
node. **Move to** changes Workflow position; `end` requests completion.
**Complete** changes Task status independently. **Retry completion** retries a
failed request without replaying the Graph.
Linear completion leaves the displayed Workflow and live Processes intact.
Loading or unavailable Workflow readings remain explicit; a failed refresh keeps
the last recorded graph with its actions disabled. Only a successful Task reading
with no captured Workflow offers **Start**, which runs `lf -b task run ISSUE`.

Use **⌘T** for a new shell tab and **View → Toggle Files** for Files.
Choose **Graph processes** in **⌘K** to open the Graph process log. It lists every Graph process in the
Task's checkout, newest first, whether an edge or a person started it; it is
not the Workflow's history. One line each: Graph, state, when it started and
how long it ran. A row opens to its id, graph and steps (the shape of
`lf graph show ID --processes --json`). A Graph whose process exited early reads
**stopped**. One whose process was killed has no exit record and keeps reading
as running. **Task details → Debug** lists the Task's raw Sessions and Processes.
Sessions list **Waiting** first and working ones in a compact group.
The Wave page's **Workflow** opens its definition in the catalogue inspector.
**Customize** or **Edit** opens the existing source editor; captured Task
Workflows stay unchanged.
**Session history** under the Graph reads nothing until expanded; it then lists that
Task's complete recorded input history (`lf usage --days 0 --task ID --json`) with each recorded
outcome. Session rows show the provider recorded on their Run and a ready summary
only when the Session recorded one.
Below the Description, **Comments (n)** is collapsed and counts the Task's Linear
thread (`lf task comment ID --json`, read when the Task is shown and
again on expanding). A failed read keeps the earlier thread marked **May be out
of date** with **Retry**; comments are never split out of the Description.
Use **Inspect** or the row's context menu for details. Presentation changes never
start a provider or resolve a Session.

The toolbar above the workspace shows its Work ancestry. On the left it drills
Wave → Task → Session: Wave and Task crumbs return to their details; the Task's
issue ID links to Linear. The Task crumb keeps the exact conversation name
without a sibling picker. Repository/Wave crumbs can choose among their Sessions. The pencil renames the Session in place through `lf session
rename`; the shown name is the shared readback, and a rejected name stays in the
field with its error. A remote Graph Session's name lives on its Machine and is not
renamed here. Outside Task navigation, the Session shows its Graph membership
beside the name. Task navigation keeps that provenance in inspection. On the right sit
the Task's read-only worktree location and the Activity glyph.

Each nonempty pane has a compact program header with its known provider,
conversation or terminal title, directory and current reported status. Hovering
the focused header reveals Close; its context menu offers zoom. Shell tabs share
one tool pane. A lone terminal omits the redundant pane header. A neutral edge and subtle shadow
distinguish the selected pane without recoloring its reported activity.
Terminal surfaces sit inside the rounded edge, so text wraps before the border
in both single and split panes.
The workspace has square top corners without an outer border. Its rounded lower
corners follow the pane radius plus its inset, keeping the gap even around nested corners.
The workspace also leaves clearance from the native window's larger lower corners.

Copy selected text with **⌘C**: the pane header briefly confirms the line count,
without decorating the selected cells. Find navigation briefly emphasizes the
current match for 160 ms; hiding a pane cancels the cue. Reduce Motion uses
Find's persistent match styling. Programs that own selection may show their own
copy confirmation; terminal feedback describes only a terminal-owned copy.

```ini
# In your Ghostty configuration; reopen Loopflow after editing.
minimum-contrast = 1
```

The default is `3`. Set `1` to preserve original colors. Correction moves only
low-contrast text toward black or white while retaining its alpha and channel
ordering. It preserves more color than replacing text with black or white, but
cannot guarantee exact hue or separation of nearly identical colors. Faint text
may not reach the requested ratio. Graphics, images, transparent backdrops and
custom postprocessing are outside the opaque-text contrast guarantee.

Files and the Graph process log open beside retained Sessions and shells in the
same multiplexer. Resize, zoom, close and Reopen preserve each pane’s content.
Drag any pane divider to resize; it highlights on hover and snaps near 38.2% or
61.8%. Showing a tool gives 61.8% to the existing pane and 38.2% to the tool.
Both sides retain usable minimum sizes, including nested splits; small windows
shrink those minimums together.

Press **⌘K** to search Waves and planned Tasks across repositories, plus Workflows
and Graphs in the selected repository. Empty search shows working Tasks first, then other Tasks, Waves and repositories; arrows select,
Return opens and Escape returns focus to the previous terminal. Graph entries
inspect the template. **Graph processes of ISSUE** opens the Task's log pane.
Each repository retains up to 20 recent
destinations for the window. Visited historical Tasks remain in recents after
leaving their pages; opening one reads its exact identity again. This does not
search all historical Tasks. Failed reads preserve the workspace and offer Retry.
If a highlighted row disappears, Return opens the first remaining visible result;
with no results it does nothing.

With an unassigned Session selected, choose **Bind to Task…** beside the Session
name. Review the named Task, then choose
**Bind permanently**. Opening the picker leaves the conversation and terminal intact.

```sh
lf open loopflow://loopflow/task/LOO-303
lf open loopflow://loopflow/task/LOO-303 --session SESSION_ID
open 'loopflow://loopflow/task/LOO-303'
```

Work links and palette Task entries open retained workspaces, including Tasks
outside the current chapter, without starting a Task Graph. An incoming address
resolves through `lf open --link --json`; the app never repeats repository
selection. A Task address opens its primary conversation. An ambiguous or
unknown address changes nothing and reports the qualified links; failed reads
keep the current workspace visible. Only its repository window receives a link.
Open Repo, Task links and the repository menu reuse one window per plan identity,
including while it opens. Different repositories keep separate queued destinations;
closing one needs no replacement window. A restored location that now selects another
plan reports an error rather than restoring its panes into that plan.
Reopening a Task keeps
the selected conversation and pane layout. Loaded repository-qualified links reuse
observed planning; successful opens do not present a Task-finding sheet.
Add `session` to open an existing Task conversation in its terminal pane. A
missing or unrelated Session leaves the current workspace intact and offers Retry.
Opening a conversation does not complete a review or start a Task Graph.

Wave details show the objective, current chapter plan/KRs, the current Project's
Workflow, current Tasks and chapter history. Completed Tasks start hidden.
**Completed** adds the last 7 days. Click **7 Days** to edit the number inline;
Enter or clicking away applies it, Escape cancels. Enter **0** for **All Tasks**
(successful completions of any age); click that label to edit again. The checkbox
hides history and remembers the range. Invalid input keeps the previous range. Canceled and duplicate history stays out of this list. Unresolved
execution and retained Session access remain available. Templates fold composed Graphs;
click a group or its disclosure control to expand it. Repeated uses disclose
independently, and both return edges remain visible at folded boundaries. Tab to
a disclosure, then use Right/Left to expand/collapse or Space/Return to toggle;
this includes nested and empty groups and XOR paths. Inspecting a return names
its target step even while the containing Graph is folded.
Task previews use the same template view until an invocation exists, including
Tasks with independent conversations. Captured invocations keep their expanded
graph. Changing a template resets its disclosure; it does not change a capture.
Projects are internal chapter records and add no navigation tier. Task
details include the directive, recorded condition, current KRs, Activity, PR and
worktree references. Choose **Edit directive**, then
**Save directive** to write through the shared PM API. Failed saves retain your
text; an accepted write with unavailable readback retains the draft and explains
what happened. Wave details expose Work outside the current plan. Failed and
partial reads remain visible in the outline.

Each terminal pane owns one native libghostty surface. A Session without a local
terminal shows its shared Open or Move here action; opening and failure states
remain in that pane. Sessions
include interactive provider Sessions and Task human GraphSteps.
Runs resumed interactively also appear, including those originally launched
headlessly. Closing their client preserves the Session and its history.

Opening a Task or selecting a Session active in another client shows its conversation
pane with **Move here**. Clicking it stops the other client and resumes the Session
in that pane. Unsent text typed in the other client is lost; the pane warns before
the click. Opening or selecting the Session leaves the other client running.

Discuss review feedback in the ongoing conversation. Closing a pane only hides
the view: the terminal and its provider client keep running and reopen as left.
Session completion controls are retired; historical completion timestamps and
feedback remain inspectable.
The shared Session projection supplies action labels, unavailable reasons and Work
paths to both CLI and Mac; local terminal presence only determines which pane to show.

Graph steps run as ordinary `lf -b skill <name>` commands under their Graph process.
The app lists, opens, and acts on the shared
Rust `SessionRecord` projection; it owns no parallel queue.
The Session ID targets conversation actions and history lookup. A prepared
input alone does not establish live provider activity.

Click the repository name to open its configured conversation. Task rows open the
Task’s retained workspace; Task details retain exact links to conversations
waiting for input, plus disclosed Session history and raw diagnostics.


Use **New conversation** to talk about the selected repo, Wave, or Task
in the configured app or terminal. It opens an interactive prompt without
creating a Task or running an autonomous operating pass. **New shell** opens
another ordinary shell tab in the Task's recorded checkout. A conversation launched here returns
to a shell when it exits or hands off to an external app.

Running shells stay intact when a Task's Session or another pane opens.

Selecting a Session in another checkout restores that worktree's conversation,
shell tabs, pane layout, and focus. Each Task keeps one Shell tool and one Files
tool; hiding a tool retains its clients and drafts. Closing a shell ends that shell. Changing a shell's
directory does not move it into another workspace.

Manually launched agents in these shells register against their actual terminal.
Selecting their Session focuses the existing shell; selection leaves both the
conversation and shell running.
External clients require explicit **Move here**. Terminals and their command titles survive Work list and detail
navigation. Switching repositories focuses that repository’s window without replacing
the origin’s panes or drafts. Native surfaces belong to
that window and are never mounted twice.
Session reads and preparation run in the opened repository rather than a
machine-wide aggregate.

Planning and Sessions share the WorkModel readings; there is no separate
Sessions-only roadmap query. A failed read keeps its last useful evidence and
exposes the error. Inspection shows the planning snapshot's generation time,
which does not establish a fresh provider sync. The outline retains an empty Wave
as an inspectable leaf; it compresses structural levels only when descendants
can take their place.

Current Wave navigation reads `lf wave list --all --current --json`. The CLI excludes
abandoned and retired registrations; unfiltered `lf wave list` retains historical
registry visibility. Authored `.lf/wave/<name>/GOAL.md` files still appear before
their first registration.

Repository scope filters the Work and Wave snapshots locally; live process
evidence remains machine-wide.
Provider activity remains separate from Task condition and human Session presence.

Loading, empty, stale-last-good, and unavailable reads stay distinct. Wave and
agent readings fail independently. A failed refresh keeps the last useful
evidence visible with its failure reason rather than painting a healthy empty
state.

Open **Portfolio** (⌘0) to choose **Folder** or **Manual** repository selection.
**Choose Folder…** changes the shared discovery folder (initially `~/src`).
**Add Repositories…** and **Remove** edit manual membership across directories.
Switching modes preserves both settings. **Refresh** reads CLI changes; returning
to the app refreshes them too. Missing paths show errors and remain removable.
**Import Saved Desktop Repositories** explicitly adopts old membership when no
shared selection exists. Opening a repository does not add it; removing membership
keeps its open conversations and files.

`lf portfolio --json` reads this same Machine-local selection. RegistryQuery carries
all reads and mutations; Swift owns no membership writer or folder scanner.

Portfolio retains the Wave inspectors. Opening a Task there routes to its
repository window rather than creating another Task workspace in a sheet.
Wave Chat loads the active backing's bounded history before SSE, keeps it visible
through reconnects, and rolls equivalent operational failures into one
disclosed notice. Cold launch does not start a chat transcript read.
Local-backed conversations compose in the app. Discord-backed conversations
mirror the same source-linked transcript and open Discord to reply; they never
create a parallel local thread. Prior backing epochs remain selectable and
read-only, and backing delivery trouble stays visible above the transcript.
Commands, tools, file edits, and loop bookkeeping stay in the journal;
decisions, deliveries, and actionable failures remain visible. The detail pane
reads the current chapter plan, Tasks, decisions, PR delivery, and Task conditions
from `lf desktop wave <wave> --json`.

Open a Task from the roadmap in its repository window, start its legal Graph,
or open its PR. **Worktree** opens the recorded checkout in Warp.
Rows show the shared condition and reason from `lf desktop plan`; unknown means
the required evidence could not be read.

Open **Go → Telemetry** for token spend, codebase growth, a token-weighted
codebase tree, and registry health.

## Product ownership

- **Sessions** own conversations and retain identity across Run replacement.
- **Waves and Tasks** appear in the work map. Linear owns authored planning;
  Loopflow's registry owns runtime state and the current chapter binding.
- **Tasks** own implementation worktrees and PR delivery and report directly
  to their Wave. The internal Project retains chapter planning and history;
  it has no separate operator.
- **Task workspace presentation** reads `lf diff --files --json`,
  `lf diff --json` and `lf file --json`.
  Lifecycle mutations remain `lf task run` and `lf interrupt`; review stages use
  the Task's persisted graph position and provider Run identity.
- **Registry queries** own durable reads. `RegistryQuery` runs
  `lf wave list/show/status`, `lf monitor`, and `lf history` JSON commands; the app does not
  maintain a second roadmap or lifecycle database. Unavailable per-Wave evidence
  renders its reason, and refresh failures leave the last successful roadmap or
  Activity history visible.


## Code map

- `LoopflowMac/Views/RepoView.swift` — one repository/Wave/Task outline and retained workspace
- `LoopflowMac/Views/SessionsView.swift` — every Session in a native split multiplexer
- `Loopflow/Models/MultiplexerLayout.swift` — immutable pane split tree
- `Loopflow/Models/MultiplexerStore.swift` — reference-owned layout, focus, zoom, and undo
- `LoopflowMac/Views/WorkActivityView.swift` — filtered durable Activity and proof links
- `LoopflowMac/WorkModel.swift` — shared readings, stable selection, and local scope
- `LoopflowMac/Views/WavesView.swift` — Portfolio membership and Wave inspectors
- `LoopflowMac/Views/RoadmapView.swift` — all-Wave roadmap and lifecycle controls
- `LoopflowMac/Views/WaveDetailPane.swift` — streamed Project plan, Tasks and metrics
- `LoopflowMac/PortfolioRepoState.swift` — one repository's Wave projection
- `Loopflow/Services/RegistryQuery.swift` — typed `lf --json` reads
- `LoopflowMac/Services/RegistryQueryLocal.swift` — local `lf` subprocess

The shared `Loopflow` target contains models, queries, and reusable views. The
`LoopflowMac` target contains AppKit, process management, menus, and other
platform behavior. The shared library remains iOS-compatible; there is no iOS
application target.

## Open Source Licenses

Open **About Loopflow → Open Source Licenses** to read and search bundled notices
offline. Matching source and rebuild instructions ship inside the app under
`Contents/Resources/ThirdPartySources`. See [packaging and updates](../third_party/README.md).

## Build system

`./dev` uses Swift Package Manager and installs to a stable application path
so macOS permissions survive rebuilds. The app queries `lf` directly and starts
only the selected Wave's `lf wave` process; it has no machine-wide service or
remote-connection mode.

The dev app supplies its bundled helper through `LF_BIN` and embedded shell PATH.
Main-Machine commands forward to installed lf; new Sessions and agent children
use its stable entry. Private-Home launches retain the bundled helper selection.
Existing providers keep their environment when reattached.

`run` and `run-debug` check `config repository --json` through that helper before
launching. This checks repository registration, not all Desktop actions. Schema
compatibility is checked by the store; ordinary commands cannot migrate the main
database. Private `LF_HOME` experiments remain disposable and explicit.

| Command | What it does |
| --- | --- |
| `uv run python scripts/loopflow-dev.py run` | Build and launch |
| `uv run python scripts/loopflow-dev.py install` | Build and install without launching |
| `uv run python scripts/loopflow-dev.py run-debug` | Build and run with stdout |
| `uv run python scripts/loopflow-dev.py build` | Build only |
| `uv run python scripts/loopflow-dev.py ghostty-build` | Rebuild the pinned patched GhosttyKit and emit its SwiftPM artifact/checksum |
| `uv run python scripts/loopflow-dev.py test` | Run unit tests |
| `uv run python scripts/loopflow-dev.py xcode` | Generate and open the Xcode project |
| `uv run python scripts/loopflow-dev.py release` | Build the release app and DMG |
| `uv run python scripts/loopflow-dev.py clean` | Remove the development app and reset permissions |

Long-running development commands write logs under `~/.lf/logs/dev/`.
The release command signs the app, hides SwiftPM's build-time resource bundles,
and requires the packaged app to render before it creates the DMG. Build
resources are restored on every verification exit.

`project.yml` generates `LoopflowSwift.xcodeproj`:

```bash
xcodegen generate
xcodebuild -project LoopflowSwift.xcodeproj \
  -scheme LoopflowMac \
  -destination 'platform=macOS' \
  build
```

The generated app target builds a validation-only `lf` helper from
the same checkout into `Loopflow.app/Contents/MacOS`. A runnable app never
borrows a different `lf` from PATH.

CI uses `uv run python scripts/build_xcode_ci.py` from the repository root.
On a DerivedData cache miss, it prepares that same helper while Xcode compiles
the app; the post-build phase still verifies freshness, copies and signs it.
`XCODE_CACHE_HIT=true` runs ordinary Xcode without background preparation.
Both builds must succeed; cancellation stops their compilers without stopping
Xcode's shared build service or compilers writing to other output directories.

Keep `Package.swift` and `project.yml` in sync.

## Shared-library boundary

- Keep Foundation/SwiftUI models and reusable views in `Loopflow`.
- Keep AppKit, Carbon, process launching, and bundled-`lf` ownership in
  `LoopflowMac`.
- Prefer whole platform files over inline `#if` branches.
- Gate platform dependencies explicitly in `Package.swift`.
- Run `uv run python scripts/check_swift_multiplatform_boundaries.py` after
  moving code across the boundary.

## Verification

```bash
swift test --package-path swift
uv run python scripts/check_swift_multiplatform_boundaries.py

cd swift
xcodegen generate
xcodebuild -quiet \
  -project LoopflowSwift.xcodeproj \
  -scheme LoopflowMac \
  -destination 'platform=macOS' \
  -derivedDataPath .build/macos-derived-data \
  -disableAutomaticPackageResolution \
  build-for-testing
```

The repository-wide gate is `uv run python scripts/test.py --all`.

```bash
lf desktop plan --all --json --watch       # the plan: Waves, Projects, open Tasks as Desktop shows them
lf desktop task LOO-1 --json --watch       # the shown Task: work, Graphs, comments
lf desktop sessions --json --watch         # the repository's Sessions as Desktop lists them
lf planning status --json --watch          # sync receipts; keeps planning exchanged
```

A window runs `--watch` reads, one process per part, through
`RegistryQuery.watchWork()`: the plan, the repository's Dashboard plans
(`lf desktop plans`), Sessions (`lf desktop sessions`) and planning status, the shown Task's status, its
activity (`lf history`) and live call trees (`lf monitor ps`). Each watch is
internally consistent; panes do not share one snapshot. Changing what the window
shows restarts only the watches whose read changed; the rest answer from their last frame without reading again.

A part arrives again only when a commit changed what it shows, so a Task created
by `lf`, a worker or another window appears without a refresh. A file written or
a commit made in a Task checkout shows within a few seconds. Sessions and the
shown Task are also read on a two-second clock, which is how a quiet Session
turns **Waiting**. Stored Workflow definitions follow the planning revision every
frame carries. File definitions are reread when the app becomes active or after
customization.

After Task actions and Workflow changes, the window waits for a checkpoint: each
running watch answers the request with a reading begun after the command, and
the checkpoint follows once all have, even when nothing visibly changed. Session
commands confirm only the Sessions watch. Explicit Retry and recovery after sleep
ask every watch to read again. If a watch ends, the window reopens all of them;
the last readings stay on screen marked unavailable until new ones arrive. Every
frame names its Loopflow home, and a build with unreleased migration drafts names
its preview copy; the breadcrumbs then end with a **Preview** chip whose tooltip
gives the path.

Task actions distinguish the running command from confirmation of its updated
reading. Confirmation expires after ten seconds; an accepted command is never
automatically repeated.

Each watch's pipes drain off the main actor. A watch speaks only on change, so
silence is not a stall; cancellation closes stdin, then terminates and reaps only
the owned process if necessary.

## Headless checks

```bash
scripts/test_desktop.sh --filter DesktopHeadlessTests
uv run python scripts/test.py --swift
```

Gate and CI build the app and inspect production views and controls without
launching a window. Display/terminal integration is opt-in with
`LOOPFLOW_NATIVE_TESTS=1`; use it for configured-host diagnostics or demo.
See [Testing](#testing) for the full commands and CLI prerequisite.

Wave opening and Retry call `WorkModel.activateProject` from both WorkSurface and
Portfolio. The command outlives the opened view; it never refreshes a view directly.
Both surfaces show “Preparing Project…” while that Wave's local command is pending,
retaining cached planning. Retry appears after failure. An unfinished persisted
Process without a pending local command remains an unknown outcome.
The Work stream supplies `project_readiness`: selected Project, dated accepted
facts, unresolved transition and the exact activation Process's outcome. Empty default
Graphs render no template section. Command transport errors remain scoped to the Wave.

Work observation and navigation project durable Waves, Projects and Tasks with
associated Sessions and Processes; they introduce no additional Work kinds.
`RepoView` shows one selected repository; `WorkModel` also serves Portfolio.
`TaskProjection` and `WaveProjection` join planning with associated conversations.
`TaskWorktreeSnapshot` describes a Task's Git worktree. `WorkspaceIdentity` and
`SessionWorkspace` retain their names because primary Sessions can use a non-Git
directory. Terminal workspaces own pane layouts. Saved cache paths, selection keys
and placement JSON fields retain their existing bytes across these source renames.

Definition search keeps Graph and Workflow destinations separate, even for the
same name. Project selectors list only Workflows. Task reads expose
`workflow_name`, `latest_graph_process`, `execution`, and `run_control` separately;
completed Graph processes retain their graph and identity. The Task watch part
supplies `graph_processes`, using the same execution details as CLI inspection.

### Inspect retained workspaces

```sh
lf open loopflow://loopflow/task/LOO-427 --diff --json
lf desktop state --json
lf task run LOO-427 --explain --json
```

Inspection reads the open repository windows through macOS automation without
launching the app, changing focus or acquiring a Session. Window incarnations,
pane layouts and hidden panes come from the retained workspace owners.
Repository, Wave and Task workspaces remain distinct even at the same checkout.
Leaving Work retains its terminals, file drafts, focus and zoom. Bare Work navigation
starts no Session or checkout. App quit still ends embedded clients. Saved pane
intent and file drafts recover after relaunch; restored shell tabs remain unattached.

Inspection reports the window's `open_work` in slot fill order and its selected
`view` plan, plus each retained workspace's `work`, title, open state, pane
`layout` and selection.
Hidden Dashboard occurrences retain their split position while visible neighbors are rearranged.
Execution locations live on contents, not workspace keys or pane receipts.
There is no outer checkout-slot layout or checkout-split menu. Graph logs use their
content location: the matching local Work stream, or an owner-pinned read with
explicit Refresh for other retained locations. They never read a peer path locally.
Task recommendations/run legality and Session actions retain their separate
source availability. Saved or failed readings supply no actions. Inspection
preserves Task source dates; Session observation time remains unavailable rather
than borrowing the UI snapshot time. This reports model state, not rendered
usability or provider readiness. Terminal panes include a `surface`
incarnation only when a native surface exists. It is separate from the pane's
content incarnation; inspection never creates a surface or cleans up an exited
child. Neither token authorizes terminal input.

`lf open` delivers the resolved Task/Session to this same repository
window. After explicit `config repository --bind`, the next open reuses the existing
scene, including one still registering. Retained local IDs come from `lf`, not
path matching. Drafts, native views and pending links stay in place; old pane
targets become stale, so inspect again before control. Restoration accepts a
retained ID but refuses a location that no longer owns it. `--diff` reveals the existing Changes browser beside the conversation,
retaining file selection and drafts. Launch acceptance returns `opening`; it does
not prove a usable native endpoint. Inspection retains the latest request's outcome:
`usable` for a Session request requires the Session's native-surface callback and a loaded Changes reading
when requested, with both original panes still visible. Closing, hiding, replacing or
zooming away a requested pane fails the pending request, as do connection, surface and
comparison errors. Navigation cancels an unfinished opening; late results cannot settle a newer
request, even for the same URL. This is endpoint readiness, not provider-turn completion
or proof of compositor rendering. A plain Task link waits for its Task page to mount;
it does not require a Session or checkout. An unavailable workspace reports failure.

Top-level `openings` records the latest repository-opening request per local locator,
including lookup/validation failures before any window registers. A plain repository
becomes usable when its validated workspace receiver registers, even while planning
loads. Registered Task links report their page/Session outcome on the window instead.
These are transient receipts, not opening history or Work identity. Remote Task links
retain the local repository window and re-observe the named execution Machine before
opening its Session and optional Changes pane. A remote link without `--session`
shows the Task without creating or connecting a Session. An explicit Changes
request without a Session still uses that Task's primary Session. Missing, changed or unavailable recorded locations
never prepare another checkout. The CLI transports native Session argv over SSH,
keeping the owner's data directory and explicit takeover behavior. Local panes and
drafts remain retained; no remote planning tree or execution record is imported.

Every Desktop gesture is a `DesktopAction` in `LoopflowMac/DesktopActions.swift`:
name (the `lf desktop` verb), title, arguments, default keys and `perform`.
`DesktopActionMonitor`, the View and File menus, the Keyboard Shortcuts window and
the Desktop socket look actions up there; nothing else mutates navigation or panes.
Keys and menus act on the shown workspace's focused pane; a socket request names an
exact window and pane, and `lf desktop` resolves those from one inspection (see the
[CLI examples](../docs/surfaces.md#drive-desktop-from-a-script)).

`DesktopServer` (`LoopflowMac/DesktopServer.swift`) binds
`$LF_HOME/run/desktop/<bundle-id>.sock` at launch (0700 directory, 0600 socket,
same-uid peers, a stale file unlinked) and answers newline-delimited JSON:
`inspect`, `perform`, `read_text` and `events`. It serves only `lf desktop` and
hosts no Sessions. An unknown action or bad argument throws `DesktopArgumentError`
(code `argument`, exit 2); a stale or missing target throws `DesktopTargetError`
(`target`, exit 3); another API version answers `version`. The receipt carries the
window's destination and the affected workspace in the same MainActor turn, so it
equals the next inspection. `events` sends the whole reading, then every quarter
second while subscribed, the windows whose reading changed.

Move retains the pane itself, including its content occurrence and surface key;
it never closes/reopens a client. `run` adds and selects a tab at the Task's
recorded location without moving focus (⌘T passes `focus=true`); a `command` line
is typed into the tab's own shell (Ghostty's initial input before the surface
starts), so the tab outlives it. Shell tabs inspect `commands`: lines submitted and
commands finished, from Ghostty's command-finished action, with the last status. Each tab
retains its location through arrangement and Close/Undo; restored intent does not
restart a process. Session panes capture their recorded location; changed
readings invalidate old pane targets without changing focus. Unknown locations
never open a local replacement. Window/content replacement rejects stale
targets. `files`, `graph-log` and `browse` select browser
tabs without changing focus or zoom. Each web page tab retains its `WKWebView`
until the tab closes; `page-back`, `page-forward` and `reload`
(⌘R, the page's toolbar) act on the addressed page. ⌘[ and ⌘] stay Desktop
navigation. Remote shells run on the recorded execution
Machine; changed or missing checkouts refuse launch without opening a local
shell. Disconnects never replay shell commands.

`lf desktop capture -p shell:1 --region selection --json` validates the retained
pane and native surface without following focus or retiring an exited surface.
Missing surfaces and nonterminal panes are explicit unavailable results, not
empty text. The verified lf3 reader writes directly into the requested byte
bound; Swift copies only that UTF-8 prefix. `screen` reads the viewport,
`scrollback` the retained screen history, and `selection` the current text or
command block (empty when nothing is selected). No unbounded extraction is used.
Mounted extraction/draft acceptance and cross-machine composition remain unproved.

```sh
lf desktop send -p session 'add to the draft'
lf desktop send-key -p session enter
```

Text preserves the draft and inserts at its cursor; Enter is separate. Control
characters in text are rejected rather than turned into keys. Both operations
require the exact existing live surface and leave focus unchanged. An active IME
composition must finish first. Missing/exited/replaced surfaces never reopen or
redirect input. A lost reply is uncertain: do not automatically resend. The native
fixture covers cursor insertion and submission but has not run here.

Sidebar excerpts use current native-turn evidence when available, with a 30-second
working-output lifetime. A current Available result shows its source age. Missing,
stale or disconnected evidence shows a labeled Task summary. Codex currently
supplies turn-qualified excerpts; providers without that evidence use the summary.
Repository and Wave navigation displays retained conversation entry without
starting a provider. Waves group their full inventory as Owned and Unowned; rows read "Jack Heart on mini" or "Unowned".


## Testing

Desktop's `swift` suite builds the app through the test-target dependency and
runs model and production-view tests through `scripts/test_desktop.sh`, which
denies WindowServer connections even on a logged-in host. No Automation
permission is needed. Only macOS's setuid `/bin/ps` leaves the sandbox so CLI
process observation still works. `DesktopHeadlessTests` checks the four Work states and
selection through the real button action. Gate and CI use this same suite.

Window/Metal/PTY integration tests are optional display-session diagnostics:

```bash
LOOPFLOW_NATIVE_TESTS=1 swift test --package-path swift --no-parallel
uv run python scripts/test.py --ui-host  # optional XCUITest diagnostic
```

Headless runs report them as skipped, not passing, and they are never
acceptance prerequisites for a headless Graph. See `release/UI_HOST_GATE.md`
for optional hosted execution and cleanup.

```bash
cargo build -p loopflow --bin lf # Real CLI transport proofs
scripts/test_desktop.sh -Xswiftc -gnone # Headless Swift tests
swift test --package-path swift --filter CatalogTests  # Catalog DTO / used-by coverage
swift test --package-path swift --filter SomeTestClass  # Filtered
```

Pass `--no-parallel` explicitly: main-actor observations share scheduling.
`WorkCLISubscriptionTests` creates and starts Tasks with `target/debug/lf` in an
isolated repository while the production Work reader is already open. It checks
sidebar updates, completion/cancellation, Back and retained workspace identity.
It launches no provider and proves no rendered native behavior.
After integrating Session kind changes, run `scripts/test_desktop.sh --filter
'TaskGraphTests|WorkspaceNavigationTests'` to compile Desktop consumers and exercise
participation and navigation fixtures together.

After upgrading GhosttyKit or its bundled shell integration, run
`scripts/test_desktop.sh --filter GhosttyShellBlockTests` to check the actual
shell output for prompt headers, command boundaries and exit status.

`scripts/prove_wave_surface_states.sh` is an optional demo capture: it launches
windows and therefore needs a display session. It is not in gate or CI.

`scripts/test_desktop.sh` runs `swift test` under `scripts/swift_test_limit.py`,
which reads the event stream and ends the whole run, naming the test, when one
test function outlives 300 s. Swift Testing's `.timeLimit` only records an
issue; a test blocked on a child process still holds the run.

On a pull request the Swift tests get 5 minutes after the build (161 s on
2026-10-22); the merge queue and main keep a 20-minute hang guard. A PR that makes
the suite slower fails on its own PR: make the slow test fast or share its setup,
and raise the budget only with a measured reason.

Failed Swift CI runs write the unfinished and slowest tests to the job summary
(`scripts/swift_test_report.py`) and retain a `swift-tests-<run>-<attempt>`
artifact for seven days with the toolchain, output log, and Swift Testing event
stream. Tests run in parallel, so the last test the log shows starting is not
necessarily the stuck one. SwiftPM buffers console output, so silence alone does
not establish that test execution never began.

The Swift transport suite launches `target/debug/lf` against a temporary Machine.
Build it before running the suite; an existing developer build can hide a missing
prerequisite in a clean checkout. CI and `scripts/test.py --swift` include this
build. `BuiltLf` owns finding and running it in a private Machine.

Transport tests register `Process.terminationHandler` before launch and await
its notification. Do not use `waitUntilExit()` after an async suspension: the
test can resume on a different thread and hang in Foundation's run-loop wait
even after the child exits. When a Swift run stops reporting progress, sample
the test helper process before changing timeouts; cleanup can be the blocker.

In asynchronous terminal tests, observe the surface after each wake-up before
checking the deadline. A busy main actor can resume after the deadline even
when the PTY produced its output in time; do not fail on a pre-sleep snapshot.

For async model readers, keep generic values crossing actor boundaries
`Sendable`. A newer local Swift compiler can accept code rejected by CI's
toolchain; record `swift --version` with compile evidence when investigating
concurrency diagnostics.
Work stream fixtures must await the fields they consume: selection membership
can arrive before planning supplies a Task's checkout.

Keep native code compatible with CI's Xcode toolchain in both SwiftPM and Xcode
builds. In particular, `isolated deinit` requires Swift 6.2; CI's Xcode 16.4
cannot enable it even with an experimental feature flag. Put teardown that can
run off-actor in a resource owner and exercise release from outside the main
actor. A newer local compiler's pass does not establish older-toolchain support.
Runtime `#available` checks do not make new SDK symbols compile on older Xcode.
Gate macOS 26 toolbar APIs behind the matching compiler condition as well;
CI's SwiftPM and Xcode builds verify the older-SDK path.
Keep Mac-only view tests in whole-file platform-gated suites, then run
`uv run python scripts/check_swift_multiplatform_boundaries.py`; a focused
Swift test pass does not check source boundaries.

SwiftPM links GhosttyKit; the Xcode project builds the terminal fallback.
Keep tests that reference Ghostty-only types or helpers inside
`#if canImport(GhosttyKit)`. Keep file-local helpers inside the enclosing
whole-file platform gate. App code outside `#if GHOSTTY_ENABLED` compiles in
the fallback too: a type it names must also live outside that block. When changing terminal code or its tests, gate both
build configurations: run the headless Swift suite and
`uv run python scripts/test.py --loopflow`. A SwiftPM pass alone does not prove
the Xcode test target compiles.

### Loopflow UI tests

Two levels, split on purpose:

**Compile check** (`loopflow` suite, in `--all` and CI). Compiles the macOS app
and its signed test runners with the terminal fallback. Swift package tests
exercise the shared suites with GhosttyKit:

```bash
cd swift
xcodegen generate
xcodebuild build-for-testing -project LoopflowSwift.xcodeproj -scheme LoopflowMac -destination 'platform=macOS' -derivedDataPath .build/xcode-derived-data -disableAutomaticPackageResolution CODE_SIGNING_ALLOWED=YES CODE_SIGNING_REQUIRED=YES CODE_SIGN_STYLE=Manual CODE_SIGN_IDENTITY=- DEVELOPMENT_TEAM=
```

CI caches this build only for identical tracked Swift inputs, Xcode, SDK and
XcodeGen versions. Matching source timestamps preserve compilation across
checkouts; changed inputs start fresh, including signed entitlement state.
Only main saves caches. PRs and merge groups still run the compile command and
build the current Rust control tools.

**Optional hosted diagnostic** (`--ui-host`). Runs `LoopflowUITests` on a
permissioned display host. It is excluded from `--all` and is never required
for Task acceptance. See `release/UI_HOST_GATE.md`.

## Dashboards

```bash
lf desktop view 'state:available'             # open an unsaved Dashboard
lf desktop view 'query:attention'             # open a saved one
```

Dashboards open only through `lf desktop`; the sidebar, Cmd-K and menus offer none.
Save an open query to name, pin and arrange it. Edit changes only that Dashboard;
repository and Wave conversations keep their normal views. Unstarted Tasks show
**Start** and their brief without opening a Session. Tasks appear side by side
only in Dashboard query results; ordinary navigation, including Command-click, selects one destination and retains
each Task's panes and drafts for its next visit.

Drag tile headers to reorder, add or remove slots, or hide an occurrence and
restore it from Hidden. Rust resolves membership and the first visible occurrence; tiles
reuse each Task's retained Session, tools and drafts. Focus never changes which
occurrence wins or replaces its primary Session face.

Task pane layouts, selected shell/file tabs and file drafts save through lfdb.
A delayed restore keeps newer tabs and edits. Conflicts leave local content intact
and show an error; **Reload saved intent** reconciles retained content before another
save. File drafts compare their saved base with disk before editing. Restored shell
intent has no launch command: a saved tab alone does not reattach a Process.
