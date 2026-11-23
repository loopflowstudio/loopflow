# Loopflow for macOS

With [Loopflow installed](../docs/getting-started.md#install), run this from
the repository root after a Swift edit.

```console
$ uv run python scripts/loopflow-dev.py run
Opening the Machine.
```

The command builds the checkout's Swift app and opens
**Loopflow Dev** beside Loopflow, using the installed server and `lf`. Both
apps show your live Waves, Tasks and Sessions. A Task edited in either app
updates in the other. Open the same running Session in a terminal and in Dev
to see its output and type into it from either view.

Each run quits the previous **Loopflow Dev** and opens this checkout's build.
The running server and Sessions keep going. Use `run-debug` to see app
logs as it runs.

Dev waits until you visit a Task, Wave, or repository to open its terminals.
On launch, even the restored selection shows a placeholder. Opening Dev beside
release Desktop leaves the running Session at its current size.

If the branch needs server features that your installation lacks, the affected
read or action reports the mismatch. The rest of the window keeps working
when both builds can connect. Test branch changes to `lf` on a
[private Machine](../docs/dev/machines.md#one-main-machine).

Repository selectors list only Git main checkouts. Linked worktrees stay visible
only through the Task Work that owns them.

Reopen a repository window: known Task names and identifiers stay readable in
the sidebar and breadcrumbs while planning loads. A returning launch opens on
the workspace saved at `<Machine>/desktop-cache/workspace.json` and shows
**Updating…** until this launch's reads replace it. Saved rows open
and navigate; Graph process state, Task condition, Session state and every action except
opening a Session wait for the fresh read. The Portfolio window opens from the
same saved workspace. With no known Task name yet, a row says **Loading Task…**
until its first read answers. The working Task list opens on the list saved at
the last launch, else shows loading until it has answered; an empty result then
says there are no working Tasks. Newly created
and reopened Tasks use their locally recorded names while tracker updates are
pending.

**Last known** describes the reading supplying the displayed name. A read that
starts failing raises one Notice for its part; the view keeps its last good
reading, or stays empty, and the Notice goes when a read succeeds.
If no name is known, the failed row says
**Task unavailable**. Desktop reconnects automatically. A successful replacement
updates the rows, retaining the selected workspace, panes and drafts.
When the app and the server are different builds and a read arrives that the
app can't decode, the Notice for that part names both and where decoding
stopped, and `lf install` brings them together. The other parts stay current:

```
The plan couldn't be read: This Loopflow (0.16.0+3500440e9) can't read what the server (0.16.0+21d541a87) sent. (missing owned_by_you at body.tasks.0)
```

**The Machine is away** means only that the connection is down.
Repeated identical Session errors leave the displayed reading unchanged; failed
per-Wave or truncated replacements keep previously readable planning, including
its saved copy. Delete the cache file to start from **Loading workspace…**.

Opening a named Task can still show its conversation opening or its own failure.
The conversation is ready when its Session accepts input; the Task name and an
accepted open request alone do not establish that readiness.

```sh
uv run python scripts/benchmarks/desktop-performance/timings.py   # how long your launches took
```

Each launch records when its first frame, saved workspace and fresh workspace
arrived, and how long each `lf` read took, under `desktop-cache/timings/` in
the Machine. Durations and command names only; the files stay on this machine.

```sh
lf machine doctor          # desktop-exit: how each bundle's last launch ended
```

When Desktop goes away, its launch journal says how: a quit and who asked, an
`exit()` and the frame that called it, a signal, or an uncaught exception. A
launch with no record ended by SIGKILL, a crash the handlers missed, or power
loss; doctor then asks the system log over the window since Desktop's last
5-minute heartbeat. Stacks, vitals, hangs, memory pressure and failed `lf` reads
go to `~/.lf/logs/lf.log` (`LF_HOME` selects another). To search the system log
yourself, call `/usr/bin/log`, since zsh's builtin `log` shadows it:

```sh
/usr/bin/log show --last 1h --predicate 'process == "runningboardd" AND eventMessage CONTAINS "loopflow" AND eventMessage CONTAINS "termination reported by launchd"'
```

Working Tasks lists owned, started, unfinished Tasks, on any Machine this store
records, in the repository's shared Task order. It is the
repository's built-in query, so opening a Task neither adds nor reorders it.
Click a Wave to scope the sidebar to its Working Tasks and subwaves; its backlog
keeps the open work nobody is running. **‹ Repo** returns to Working Tasks. Unstarted Tasks
open their brief beside **Start**, which runs the Task's Workflow; opening alone
starts nothing. Waves follow Tasks in the same scrolling sidebar, with counts
from the planning inventory.

**New Wave**, under the sidebar's Waves or in the Portfolio, is one form: a
name, and a repository picker when there is more than one to choose from.
**Create** adds the Wave; with no repository, Create is off.

**⌘N** opens New Task. Type what you want and choose **File to backlog** to
open its brief, or **Start with `<workflow>`** to open its conversation. The
sheet shows **Filing…** or **Starting…** while the Machine answers. On Start
admission, the Task appears in Working. Its conversation shows your request,
shows setup progress, then accepts input.

```
Fix the flaky reconnect test
Checkout ready  0.2 s
Graph code started
```

The checkout branches from the default branch as of Loopflow's last background
fetch; the Task's first sync catches up. A setup step that fails prints its error
in the pane, and the agent still starts once the checkout exists. Reopening the
conversation continues setup from the step that stopped; it never starts a
second Graph. A confirmed refusal leaves the draft open to edit and submit
again. After admission, a failure to open the conversation shows in the Task's
pane with Retry.

Click a row to select its retained workspace. The title-bar **Repo › Wave › Task**
path follows hierarchy: click a name to go there, or the **›** after it to list and
search what it contains; Back and **⌘[ / ⌘]** follow visits. **⌘K** searches Work
across repositories and opens another repository in its own window. Its
**Sidebar shows** picker filters Working tasks / Available / Busy without
changing the selected Work. Click a Wave and its name moves into the
sidebar header; the rows below keep that same relative Task order until
**‹ repo** returns to Working Tasks. **⌘1–9** selects the sidebar's Task rows by
position and **⌃Tab / ⌃⇧Tab** cycles them. Missing slots do nothing. Selecting a
Dashboard's Task, from any of these, keeps the Dashboard in the path, as its tile does.
With sidebar focus, **⌘F** searches its visible Tasks and Waves; **Search all Work**
widens the same navigator. **Shift-F10** opens the focused Task's actions without
changing the open workspace.

The Task action menu moves work into a Wave or back to **No Wave** through the shared planning operation, retaining its workspace and drafts.

Open an active Task whose planning state is **Todo**: wherever its lifecycle
appears in the page, sidebar, board or breadcrumb picker, it shows **Active**.
Planning state remains editable in Task details. Once work has started, its execution determines the lifecycle
shown across Desktop. Completing or canceling a never-started Task instead shows
it as completed or abandoned and offers **Reopen**.

Ownership is separate. Unassigning a started, unfinished Task puts it in **Backlog** while
keeping its lifecycle, checkout, PR and conversations. **Start** and **Unstarted**
apply to work that has never started; an unassigned running Task remains Active.

A Task is **Available** when one of its conversations is available for your next
turn, and **Busy** when a current activity report says it is working. An open
conversation with unknown activity adds neither. With an available conversation
and a busy one, the Task matches both filters; its board badge shows Available.
The board and sidebar use the same activity reading. These badges describe
conversations; the Task's lifecycle and NOW condition remain separate.

Task conversations include Sessions bound to the Task and Sessions working in
its checkout. Repository and Wave primary conversations keep their own scope,
even in a Task checkout. The Task's conversation list and breadcrumb Session
picker use the same membership.

A Task's row is its whole title, wrapped rather than cut, then its ID and one
status word. A row that needs someone adds one line saying why:

```text
Loopflow for Mac's Task workspace matches
the design prototype
3d4350  Blocked
Approve the plan?
```

The word is the first of these that applies:

1. What the Task's program reports: **Busy**, **Available**, **Blocked**,
   **Done**. A blocked program's line is what it asked.
2. **Blocked** or **Waiting**, when the Task's condition says so and no
   program reports. The line is the condition's reason, the one
   `lf plan list` prints. A Task whose Graph has never run waits on nobody,
   opened or not: it reads its lifecycle word and its row offers Start.
3. Its lifecycle word: **Unstarted**, **Idle**, **Active**, **Done**,
   **Abandoned**.

A Task whose condition could not be read adds no word for it. The row is the
same in the sidebar, the title bar, a Dashboard tile's header, the roadmap,
the Now list and a Wave's Portfolio page, and the Task's own page and the
jump menu print its word, so a Task never says two things about itself. The
Now list holds the Tasks whose rows say they need someone, under that word. The Portfolio has no program reports, so its rows start at the
second rule. Click the row on any of them and the Task opens.

The selected row is filled; rows carry no stripe or border. A Wave's count
beside its name is in the same face as its name.

Drag a Task in the sidebar to change its shared order. Ordinary repository and
Wave lists, boards and Task pickers keep that relative order after refresh.
Filtering or opening a Task leaves the order intact. Saved Dashboard arrangements,
search results and Done's completion-date ordering keep their own order.

Each Work keeps one pane layout and document store. The browser offers Brief,
Diff, Files, Task & workflow, Graph processes and web pages. Rex, Herdr and cmux
open only a Task's conversation and never read Desktop's layouts.

Repository and Wave
entry shows their conversation, opened on load or on selection.
Waves pair that conversation with their objective, status, planning and
full Task inventory, followed by their metrics. **Project history** and **Workflow**
open the existing planning inspectors. The sidebar starts at 240 points wide;
drag its edge to resize or use the title-bar button or **⌘\** to toggle it.
Its scroll position survives Wave and Task navigation. Working order survives
relaunch; ordinary launch restores the selected Work.

Closing a repository window keeps its panes, selected Work and file drafts until
Desktop quits. Reopening a known repository shows retained content immediately,
even while repository resolution waits or fails. Errors remain visible; new Task
links wait for resolution before delivery. Unknown repositories need an initial lookup.

Release Desktop opens conversations on load for the repository, each Wave, and
every claimed working Task (idle, active or moving), even on a first visit.
Selecting one shows its opening progress, the live conversation, or a failure
with **Retry**. Desktop prepares two conversations at a time, starting with the
selected Work, then visible tiles, the repository and Waves, and recent working
Tasks. The window renders while they open.
A prepared conversation starts its provider at once, before any pane shows it;
selecting the Work shows the running terminal. One **Opening… Input paused**
reading remains through preparation, attachment and native input readiness. The
terminal owns that display once mounted; there is no second workspace overlay.

Recent unfinished Tasks stay warm for immediate return: Desktop retains sixteen, plus
any additional panes on screen; a Task admitted from a Dashboard tile stays warm after
the Dashboard leaves. A board's other pages wait: past the bound, a tile opens when its
page turns up or it is selected, and a tile that leaves returns to the bound.
Repository and Wave conversations stay warm beside that bound.
Return lands at the bottom. Older conversations reconnect or resume the same
provider history automatically. `lf open --session <ID> --host desktop`
opens the exact conversation, including repository and Wave conversations without a Task. After relaunch, the saved selected Work opens
first, then the bounded warm set reconnects with at most two preparations at once.
Fresh completion and recorded Machine/checkout evidence must arrive before
background retention. Close removes warm intent; refresh does not reopen it, and a
deliberate selection does. Completed Tasks become eligible for cleanup after
fifteen minutes without a Session viewer, once live work settles.

Automatic opening creates the primary Session if absent and starts its native
conversation if it has no history. Existing history resumes on the same Session.
No Start or Open click is needed, including after relaunch, and a started Task
opens whether or not anyone owns it. An unstarted Task has no checkout and no
conversation: its Session slot offers **Start**, which runs its Workflow. Opening never launches a Graph, claims or starts a Task, runs the
repository or Wave operator, or replays consumed input: with no request, the first
turn of any primary reads its scope and reports where it stands. A failed
automatic opening shows its error with **Retry**. A finished Task opens by itself
only when it already has a conversation; otherwise **Open conversation** starts one.
Every opening is `lf resume`: a held terminal gains a view, an ended Session resumes
from its history, and a provider running in a native client lf can't share is taken
over, ending that client. A pane's button carries Rust's label for which it will do
(**Reconnect**, **Open here**, **Take over**, **Try again**, **Start conversation**).
Every view of a held terminal can type into the same draft. Desktop runs the
server’s command with `LF_RELAY_FOCUSED=0` in the pane environment. Older CLIs
can run the same command. With a relay and holder that support initial focus,
a background pane leaves the terminal’s size and colors alone. Typing in a pane
takes ownership, as does focusing it before the first key. When the owner
closes, the most recently used live view takes over.
Close and eviction discard late opening results.

Finite CLI requests time out after 30 seconds. Canceling preparation ends only
that request; running Sessions continue. If a change times out, refresh to learn
its result before retrying.

Session panes receive their relay's status and send Retry over one
private Unix connection. Quiet streams schedule no reads, including in retained
hidden panes. Status loss pauses input while the relay continues; a fresh status
connection supplies the current reading without replaying actions. Closing a pane
stops observation and removes its private socket directory. Input waits for a
connected reading with native readiness; a saved screen cannot enable input.
The terminal view tells Ghostty when it gains or loses focus (first responder in
the key window); the relay turns Ghostty's focus reports into the holder's
`focus` requests, so the pane being looked at sets the size.

Pane headers name the known provider, conversation and directory; arbitrary terminal
titles remain context under **Shell**, without guessing which program is running.

Open a Task's **Graph processes** page to see its current position and readiness,
then expand a run, loop position and step for recorded timing and exit evidence.
Earlier runs stay collapsed. **Files** opens the diff; steps without a recorded
summary show no invented summary. Local and remote run pages follow store changes.
Open native Task Sessions also receive settled progress through their provider,
without typing into the draft. Codex and Pi keep the incoming context hidden;
Claude shows a collapsed peer message and OpenCode a brief worker-progress notice.
The provider schedules delivery around active turns; uncertain delivery is retained
instead of replayed after reconnect.
Progress posts name the runs that changed, including an earlier run that finishes
or fails while newer work is running.

Task IDs in the sidebar link to the issue when its URL is recorded; otherwise
they remain plain text. The link opens separately from the Task row. Unknown
activity and live clients without reports show the Task's condition or
lifecycle word. The PR link
and owner are in the Task pane, not the row.

Follow a conversation in the pane and sidebar: Busy for a working report,
Available for an idle report, Blocked for a question or permission request, and
Done for a reported result. Both labels use the same report wording. A pane
describes its own conversation; a Task's Available/Busy filters and board badge
aggregate all its conversations in Rust, so a busy pane can belong to an
Available Task. Reported Done is a program result, independent of Task lifecycle.
Reconnect restores the current report without sending another prompt. Quiet output
does not change activity; clearing a report removes its label without completing
the Task.

Work selection uses the theme’s selection fill and contrasting text.
File selection keeps a neutral fill and edge; keyboard focus has a separate outline.
Ordinary controls and links use text roles; the theme’s accent emphasizes deliberate actions.
Status labels and shapes retain their meaning across palettes.
Choose **Settings → Appearance** to preview the whole workspace. Pick
**System**, **Light** or **Dark**; System follows macOS and lists both sides,
Light or Dark lists only its own. Each side's heading names its chosen theme
(**Light themes · Fjord Light**), then shows built-ins as cards and **Yours**,
ending with a dashed **Import file…** tile. An imported file joins the side its
own colors say; in Light or Dark mode the preview switches to that side. Every card is drawn in its
own theme: a title bar strip, its name as a heading, Busy, Blocked and Error,
ANSI 1–6, a selected row and a **Run** button:

```text
▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔
Fig
Busy  Blocked  Error
● ● ● ● ● ●
[Selected row] [Run]
```

Families sort by name after Loopflow, so a family's light and dark halves sit in
the same column. Clicking a card previews it in every window; the chosen card is
ringed in the window's focus color. A theme with unreadable roles shows why
under the cards, before **Apply**.

```yaml
# ~/.lf/config.yaml
theme: "light:Folio,dark:Fig"   # System
theme: "Fig"                    # Dark (or Light) mode: one name
```

**Apply** rewrites only that line; **Cancel**, Escape or closing writes nothing.
Edit it by hand and every window follows; a misspelled name keeps the last theme
and names the fix. Absent means Loopflow Light/Dark. A repository's
`.lf/config.yaml` never sets the theme.

Built-ins are 12 families, each a light and a dark theme. **Loopflow Light** keeps
the original cream/burgundy; **Loopflow Dark** is blue-grey with rose headings and
links, under the same burgundy title bar. The other eleven are generated from
specs in `scripts/theme_catalogue.py`: **Fig** is plum with a lime accent, its
light half **Fig Leaf** a leaf-green ground with plum ink.

Your themes are Ghostty theme files in `~/.lf/themes/`; `~/.config/ghostty/themes/`
is read too, never written. Names resolve to yours, then built-ins, then Ghostty's.
A file named like a built-in overrides it. **Customize <name>**, beside Cancel and Apply,
acts on the card chosen last: on a built-in it writes your copy; on yours it edits in
place. **Delete <name>** removes yours (**Revert <name> to built-in** for an override).
**Import file…** copies a theme into Yours on the side its colors say; in Light or
Dark mode the preview switches to that side.

```
background = #171924
foreground = #cad2ef
palette = 4=#7aa2f7
# Optional app roles; the rest derive from ANSI colors.
loopflow-accent = #3d59a1
# Optional; without it the bar follows the accent.
loopflow-titlebar = #242f50
```

Files read up to 256 KiB of UTF-8. Six-digit RGB background/foreground are
required; cursor, selection, `palette = N=#RRGGBB` and `loopflow-` roles are
optional. Preview explains defaults, repeats and omitted settings. Fonts,
commands, keybindings, includes and theme references are never followed.

The title bar is its own role. A file that leaves it out gets a bar from its
accent, darkened where needed (lightened under black text) so the bar's text
stays readable; a theme you made before the role existed may show a slightly
darker bar. **Customize** lists **Title bar** under Actions and focus, and saving
states it: after that, changing **Button** leaves the bar alone.

Unreadable app/default/selection/cursor text prevents Apply; ANSI colors keep
their source values. Personal non-color Ghostty settings still load; its `theme =` is ignored and its file never written.
Source and license notices ship offline with the catalogue.

Under the jump bar, the workflow bar reads where the Task stands:

```text
code  plan  build  👤ship-review  deliver  👤verify   build is running implement. You are next needed at ship-review.   [Auto-Ship]  …
```

The Graph's name, then its top-level steps in order, a person before each step
that waits for you. Steps already passed are quieter; the step the Task stands
in is strong, in the working colour while it runs and in the waiting colour
when it has stopped or is waiting for you. A step that appears in more than one
part of the Graph (`concept-review` runs in both `plan` and `build`) marks the
part it is running in. The sentence is the one `lf task status` prints. In a
narrow window the steps go and the sentence stays.

When the Task can move, one button says how: **Start** before it has run,
**Run** once its Graph has stopped. **Auto-Ship** turns the Task's auto setting
on and runs it, as `lf task auto on` does: its run stands in at each
interactive step and lands the PR. Turning it on raises the Notice "LOO-5
Auto-Ship requested · Undo"; Undo is Stop Auto-Ship (`lf task auto off`), and
so is the button, which reads **Stop Auto-Ship** while auto is on. While auto is on,
the sentence names auto instead of you, and the Task's board card and sidebar
row carry an Auto badge. Open a Dashboard with `state:auto` to watch those Tasks.

Everything else is in **…**, the same list as the sidebar row's context menu:
**Start from…** (or **Start with…** before the Task has a Graph), Complete or
Reopen, Claim or Unassign, Move, Copy ID and link, and **Abandon…**.

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
remains available when the Work has no conversation open.
Shell holds retained shell tabs. The Task's pane has one row of tabs: **Brief**,
**Diff** and **Files** (Option-Command-1/2/3), then anything else open beside
them. Hidden tools keep their clients and drafts.

**Brief** opens on the Task's title, then one sentence with the word its row
says when no program reports, its step and why
("Moving, at build. The loop chose another pass on the concept review.",
"Waiting, at demo. ready."; a Task that needs nobody gives no why when it
would only repeat its condition: "Idle."), a line for what it
needs when that sentence has not already said it ("Blocked: checkout has uncommitted changes"), then its PR, the Tasks the brief names
(click one to open it) and an outline when the brief has headings or an
`Acceptance:` lead, as a two-column table that wraps in a narrow pane. Its
scroll position survives page and Task switches in this window; a brief that
changed since you last read it says so above its text.

**Diff** opens on how much changed ("4 changed files", with the **Whole Task**
step menu beside it), then what is compared: the base branch and its commit,
wrapping rather than cut. The changed files sit beside the
whole comparison as one stream. `lf task changes` returns each file’s change count with the list,
before the pane reads any patches. Each shown file's lines are read on their own,
those near the top of the pane first, so a large file never cuts another short
and a hidden file is never read. A new file reads as its contents. Size
collapses nothing: a line past 1,000 characters shows its start and **N more
characters · Show**, a file past 1 MB shows its first 1 MB and says it was cut,
and a file that could not be read offers **Retry**. Code is colored by its
language's grammar, here and in Files, in your theme's terminal colors:
keywords, types, functions, properties, strings, numbers and comments each
have their own, readable on every row. A hunk is colored by its own lines
first and by the whole file once both sides are read, so a hunk that opens
inside a Python docstring colors as a docstring. Swift, Rust, Python,
TypeScript, JavaScript, Go, C, C++, Objective-C, C#, Zig, Ruby, Java, Kotlin,
SQL, shell, HTML, CSS, Markdown, YAML, TOML, JSON, INI, Dockerfiles, Makefiles
and justfiles have grammars; other files stay plain. The page fits a pane 360 pt wide: the toolbar keeps the
sidebar toggle, the stepper and the **…** menu, which holds what has no room,
layout first, named in words (**New code**, **Unified**, **Side by side**), then order and
tour, then Focus, and **Wrap long lines**. **Whole Task** opens a menu of the
Graph steps in the branch's
commits since its base: a run of commits whose subjects start `lf implement:`
is one step, any other commit stands alone, and picking one compares that
step's commits. A button by the title says where the work is
delivered and opens the delivery card, whose link reads **Open on GitHub**. Loopflow's record decides whether there
is a PR: none, publishing, open, merged or abandoned. GitHub's reading adds
checks, review and description with its age; a failed, stale (over two minutes)
or behind-the-checkout reading stays readable but loses its colors, and merged
never reads as a finished Task.

The files read in the tour's order when the checkout has
`scratch/tour.yaml`, which the `tour` skill writes (`lf help tour`):

```yaml
at: 81d3fa5973e63a4394a25357502e635a3baf5d5b
first:
  - path: swift/LoopflowMac/TaskFileDocument.swift
    why: Each file's lines are read on their own
hide:
  - path: docs/
    why: Path literal renamed, nothing else
```

`first` puts those files at the top in that order, each `why` heading its files
in the stream and the sidebar. Every file it does not name follows **By group**,
by path within each group. Without the file the Diff reads By group. The order menu switches between **Tour**, **By group** and **By path**.
`hide` leaves those paths out for this change only; the sidebar lists each with
its reason and **Show**, and they never join the hidden paths below.

The groups are the `diff_groups` setting, an ordered list read as `lf`
resolves it (`lf config explain diff_groups`). The nearest layer that sets it
wins whole: yours for the repository, then your global one, then the
repository's `.lf/config.yaml`, then the default. A file goes under the first
group with a matching pattern; files no group matches go where the list says
`rest`, or last when it has none:

```yaml
diff_groups:
- title: Docs
  paths: ['*.md', /docs/]
- title: Data model
  paths: [/migrations/, '*.sql', schema, /models/, '*.proto', dto]
- rest: Everything else
- title: Tests
  paths: [/test/, tests/, _test., .test., .spec., tests.]
- title: Generated and lock files
  paths: ['*.lock', '*.snap', /generated/, /vendor/]
```

A pattern matches `/` followed by the file's path, ignoring case: anywhere in
it, or at its end when the pattern starts with `*`. That is the default, a
little shortened; `lf config catalog` prints it whole.

`at` is the commit the tour was written at. Once a later commit changes a file
outside `scratch/`, the stream says so ("The tour was written at 81d3fa5 · 3
files changed since") and the toolbar offers **Update tour**; with no tour it
offers **Write tour**. Either runs the `tour` skill headless in the Task's
checkout and reads in Tour when the new file lands. Until then the stream
shows "Writing the tour…" with the run's log path, and the button stays beside
it: nothing reports the run ending, so a run that died is started again with a
click. Files changed since `at` still follow the named ones. Nothing rewrites
the tour on its own, and a Task on another Machine offers neither button.

The sidebar lists the files in that order: type to narrow the stream, click a
file to bring it to the top. The stream draws about 1,500 rows from where it
starts, more as you scroll down, so a jump lands at once however far it goes:
a jump past that starts the stream at its file, with **N files above · Show**
over it. Under them, **Always hidden** is the `diff_hidden`
setting, patterns matched the same way. Every layer adds to it (`scratch/` by
default; `[]` clears the layers below), and each pattern is named by the layer
that added it. Only yours can be removed or added there; they are saved in the
repository's `.lf/config.local.yaml`. Hidden files stay in the count: "4 of
9 changed files". Below 620 pt the sidebar opens over the stream from
its button and closes when you choose a file.

The stream follows the comparison live without moving what you are reading. A
bar above it names the file you are in, with its counts, **Open** and
**Viewed**, and under it the function, type or Markdown heading the top line
sits in. **Viewed** folds a file to its header until its lines change; the
sidebar counts "12 of 60 viewed" and marks each folded file **Viewed**.
Press `]`
and `[` (or the arrows around the position) to step through changes across
files, skipping folded files. The change you land on pulses once. **Focus** shows one file and still
steps into the next. Between hunks the stream says how many lines it skipped
and which declaration follows; click that row to read the lines in place,
in a step's comparison as in the whole Task's.
Edited lines emphasize the changed words. A block moved within a file collapses
to one row, "Moved here from L12", that expands to its old and new lines.
The stream opens on **New code**: the file as it is now, removed lines folded
to "N old lines · Show". **Unified** shows both; **Side by side** needs a
stream at least 640 pt wide.

Markdown, images and HTML open rendered. A changed `.md` reads as a redline:
added words filled, removed words struck, a new paragraph filled whole and a
deleted list item struck whole. **Side by side** puts the old document left
and the new one right, block beside block. A changed PNG, JPEG, GIF, WebP or
SVG shows old and new side by side, each with its pixel and byte size
(`1200×800 · 84 KB`); an added image says "Added" on the old side. A changed
`.html` shows both pages live, side by side, each scrolling on its own and
neither reaching the network. Images and pages stay two panes in every
layout, stacked old above new in a pane under 640 pt. **Source** in a
Markdown, SVG or HTML file's header shows its line Diff instead, until you
switch back or close the window. An image too large to show says so with its
size, and one that can't be decoded says that; the other side still shows.

Click a line number (Shift-click extends) to write a comment under those lines;
Return keeps it there. **Send to conversation** (Command-Return) puts every
comment into the conversation's unsent input as one line, each naming its file,
lines and side, and sending stays yours. A comment follows its lines when the
worker edits above them; when the lines themselves change it says so, keeps its
place and is still sent. Comments last as long as the window. **Copy** takes
the selected lines without patch markers, as does a line's context menu, which
also copies the selection or the new code shown; selecting text never takes
the + and −. **Reference in conversation** types
`path:L4-6 (after, working tree on <sha>)` alone. Files' Diff has the
same comments, skipped lines and Send.

**Files** holds open documents and their Read, Edit and Diff controls. One menu
names the open file and, once several are open, where it sits among them
(**TaskRow.swift** 1 of 3), and switches between them or closes it; under it,
the Machine as a place (This Mac), then the checkout and folder in mono. Files starts with
its chooser closed. Click the file-browser toggle or press Command-P for
**scratch** and the Task's checkout tree, read from the Task's Machine as you
open folders, 500 entries at a time (**Load more** for the rest). `M`, `R` and
`U` mark files changed from the branch's recorded parent commit; `•` marks the
folders holding them. ↑/↓ move, → opens a folder, ← closes it, Return opens the
file. Search matches scratch, the folders you have opened and changed files. A
folder that can't be read says why under it, with **Retry**. Expanded folders
and scroll stay put across page and Task switches. Selection closes the dropdown; pin it to keep the list alongside the document
(Control-Command-B). Escape dismisses the dropdown. Sidebar selection switches
the whole Task workspace; selections inside a Task can focus its companion.
Resizing or closing the browser is remembered while the window stays open;
layouts are not yet restored after quitting the app.

Open **•••** or press Command-/ for the selected Task’s actions. Row context
menus use the same actions for the clicked Task. Command-T adds a shell tab. The main Session's menu switches between **Claude**,
**Codex**, **OpenCode**, **Pi** and **Antigravity**, continuing the same Session with its history.
**Refresh session** reconnects the pane to that conversation, including after a
failed launch or stale connection. It keeps the provider's history and running
process, and takes over a conversation a native client holds. The control
stays visible when the Session is the only pane. The same action is available
through `lf desktop do refresh-session -p session`.
**Reset session** starts a fresh provider thread in the same Session, seeded with
its saved conversation. Reset and provider switching first read that history,
then replace the current provider process. If the
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
⇧⌘T restores the last closed tab's identity and history. Close ends its shell
and jobs; New Shell starts fresh execution.

**Read** is where documents open: Markdown renders formatted, HTML as a live
page that runs its scripts but never reaches the network, and an image as the
picture, whatever its extension. Links open in the
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
Remote paths are never opened or subscribed on the presentation Mac. Filesystem events
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
panes; **Task details** holds its description. Opening a conversation a native
client holds takes it over here; that client ends.

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
Missing planning or selection readings retain known Work and raise a Notice.
**Task details** opens the description, comments and Run history.

Opening a Task enters its retained panes and opens its conversation.
Navigation never starts its Graph. Returning preserves panes, drafts and focus, including historical
conversations. Explicit bare CLI Task links still support inspection without
conversation acquisition.
The workflow bar shows where the Task stands and what it can do next. Opening
feedback stays inside the terminal area. Resolving a conversation installs its
record and pane before ending preparation, including explicit Session links.
Cached switches reuse their panes and drafts without preparing again.
When the selected Task's lifecycle becomes completed or abandoned, it stays
visible with a green **Done** or red **Abandoned** header. Navigating away restores the ordinary
sidebar filters. **Back** (⌘[) returns to the retained workspace and its sidebar
row, preserving conversations, files and drafts without starting execution.
**Start from…** in **…** lists `start` and the Task's interactive steps; picking
one runs `lf task run ISSUE --from STEP` as a child of the app. **Complete** changes Task status independently: the row leaves the list in the
click and the next Task opens. A refused completion brings the row back with a
Notice saying why; completing again never replays the Graph.
Linear completion leaves the displayed Workflow and live Processes intact.
Loading or unavailable Workflow readings remain explicit; a failed refresh keeps
the last recorded graph with its actions disabled. Only a successful Task reading
with no captured Workflow offers **Start**, which runs `lf -b task run ISSUE`.

Use **⌘T** for a new shell tab and **View → Toggle Files** for Files.
Choose **Graph processes** in **⌘K** to open the Graph process log. It lists every Graph process in the
Task's checkout, newest first, whether an edge or a person started it; it is
not the Workflow's history. One line each: Graph, state, when it started and
how long it ran. A row opens to its id, graph and steps (the shape of
`lf graph show ID --json`). A Graph whose runner exited or was killed reads
**stopped**. **Task details → Debug** lists the Task's raw Sessions.
Sessions list **Waiting** first and working ones in a compact group.
The Wave page's **Workflow** opens its definition in the catalogue inspector.
**Customize** or **Edit** opens the existing source editor; captured Task
Workflows stay unchanged.
**Session history** under the Graph reads nothing until expanded; it then lists that
Task's complete recorded input history (`lf session usage --days 0 --task ID --json`) with each recorded
outcome. Session rows show the provider recorded on their Run and a ready summary
only when the Session recorded one.
Below the Description, **Comments (n)** is collapsed and counts the Task's
thread of steers and reports (`comments` in `lf task show ID --json`, read when
the Task is shown and again on expanding). A failed read keeps the earlier
thread marked **May be out of date** with **Retry**; the thread is never split
out of the Description.
Use **Inspect** or the row's context menu for details. Presentation changes never
start a provider or resolve a Session.

The toolbar above the workspace shows its Work ancestry. On the left it drills
Wave → Task → Session: Wave and Task crumbs return to their details; the Task's
issue ID links to Linear. The Task crumb keeps the exact conversation name
without a sibling picker. Repository/Wave crumbs can choose among their Sessions. The pencil renames the Session in place through `lf session
rename`; the new name shows when you save, and a rejected name returns with a
Notice saying why. A remote Graph Session's name lives on its Machine and is not
renamed here. Outside Task navigation, the Session shows its Graph membership
beside the name. Task navigation keeps that provenance in inspection. On the right sit
the Task's read-only worktree location and the Activity glyph.

Each nonempty pane has a compact program header with its known provider,
conversation or terminal title, directory and current reported status: one
dot in the status's colour and a short phrase.
When a Task's conversation starts fresh rather than resuming, "New
conversation; earlier one left behind" sits on its own line between the header
and the terminal, never over the terminal's text. Hovering
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

Terminals draw in Spline Sans Mono 13 with its real bold and italic faces and
ligatures off; Ghostty draws box drawing, blocks and braille itself, so Claude's
frames stay continuous. Your Ghostty configuration still wins: a
`font-family` you set comes first and Spline Sans Mono fills the glyphs it
lacks, and your `font-size`, `adjust-cell-height` or `font-feature` replace
Loopflow's.

```ini
# In your Ghostty configuration; reopen Loopflow after editing.
font-family = Berkeley Mono
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
outside the current plan, without starting a Task Graph. An incoming address
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

Wave details show the objective, the plan and its KRs, the Wave's Workflow,
current Tasks and plan history. Completed Tasks start hidden.
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
A Wave's planning adds no navigation tier. Task
details include the directive, recorded condition, current KRs, Activity, PR and
worktree references. Choose **Edit directive**, then
**Save directive** to write through the shared planning API. Failed saves retain your
text; an accepted write with unavailable readback retains the draft and explains
what happened. Wave details expose Work outside the current plan. Failed and
partial reads remain visible in the outline.

Each terminal pane owns one native libghostty surface. Opening, failure and
recovery actions stay in that pane. Sessions include interactive provider
Sessions and Task human GraphSteps.
Runs resumed interactively also appear, including those originally launched
headlessly. Closing their client preserves the Session and its history.

Opening the pane takes input from a native client, which ends; a held terminal
only gains this view.

Discuss review feedback in the ongoing conversation. Closing a pane only hides
the view and releases its Desktop hold. Shared lifecycle protections determine
how long the provider stays running; conversation history remains available.
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
Terminals and their command titles survive Work list and detail
navigation. Switching repositories focuses that repository’s window without replacing
the origin’s panes or drafts. Native surfaces belong to
that window and are never mounted twice.
Session reads and preparation run in the opened repository rather than a
machine-wide aggregate.

Planning and Sessions share the WorkModel readings; there is no separate
Sessions-only planning query. A failed read keeps its last useful evidence and
exposes the error. Inspection shows the planning snapshot's generation time,
which does not establish a fresh provider sync. The outline retains an empty Wave
as an inspectable leaf; it compresses structural levels only when descendants
can take their place.

Wave navigation reads `lf wave list --all --json`. Desktop and `lf wave list` read
the same rows: every registered Wave that `lf wave remove` has not removed. A
removed Wave stays readable by `lf wave show <id>`.

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

`lf desktop portfolio --json` reads this same Machine-local selection. RegistryQuery carries
all reads and mutations; Swift owns no membership writer or folder scanner.

Portfolio retains the Wave inspectors. A click on a Task's row there, on the
roadmap, the Now list or a Wave's page, opens it in its
repository window rather than creating another Task workspace in a sheet.
A Wave on the roadmap is a section under its name, divided from the next by a
line. A Wave's page shows its name, objective and key results, every Task, then
its metric table. A metric whose contract has an issue shows a notice above the
table with the count and each issue's summary; **Copy** copies the summaries.
Wave Chat loads the active backing's bounded history before SSE, keeps it visible
through reconnects, and rolls equivalent operational failures into one
disclosed notice. Cold launch does not start a chat transcript read.
Local-backed conversations compose in the app. Discord-backed conversations
mirror the same source-linked transcript and open Discord to reply; they never
create a parallel local thread. Prior backing epochs remain selectable and
read-only, and backing delivery trouble stays visible above the transcript.
Commands, tools, file edits, and loop bookkeeping stay in the journal;
decisions, deliveries, and actionable failures remain visible. The detail pane
reads the Wave's plan, Tasks, decisions, PR delivery, and Task conditions
from `lf wave plan <wave> --json`.

Beside each Task's row, **Start** (**Run** once its Graph has run),
**Open PR** and **Worktree** start its legal Graph, open its PR, and open the
recorded checkout in Warp.
Rows show the shared condition and reason from `lf plan list` as their status
word and line; an unknown condition adds no word, because
the required evidence could not be read.

Open **Go → Telemetry** for token spend, codebase growth, a token-weighted
codebase tree, and registry health.

## Product ownership

- **Sessions** own conversations and retain identity across Run replacement.
- **Waves and Tasks** appear in the work map. Linear owns authored planning;
  Loopflow's registry owns runtime state and the current planning binding.
- **Tasks** own implementation worktrees and PR delivery and report directly
  to their Wave. The Wave's planning row retains the plan and its history;
  it has no separate operator.
- **Task workspace presentation** reads `lf task changes --json`,
  `lf task diff --json` and `lf task file --json`.
  Lifecycle mutations remain `lf task run` and `lf task interrupt`; review stages use
  the Task's persisted graph position and provider Run identity.
- **Registry queries** own durable reads. `RegistryQuery` runs
  `lf wave list/show/status` and `lf session history` JSON commands; the app does not
  maintain a second planning or lifecycle database. Unavailable per-Wave evidence
  renders its reason, and refresh failures leave the last successful planning or
  Activity history visible.

## Work tracking

The sidebar presents the repository's retained working Task order, then direct Wave
rows. Opening started, unpaused Work appends once. Activity and query refresh never
reorder slots; explicit pause hides a slot without deleting its stored position.
Working references are saved through WorkCache; they convey no execution authority.
Unstarted Tasks show their brief beside a Start offer in the Session slot without
joining the working set. Only Start runs anything.

The navigator's Working tasks / Available / Running choice filters presentation
without selecting Work or changing panes, clients, focus or drafts. Missing planning
keeps an unavailable row. Work observation retains its last reading on failure.
Wave navigation leaves working positions and the sidebar viewport intact.

Title-bar breadcrumbs follow hierarchy as a jump bar: each name navigates, and the
› after it lists what is inside (repository › Waves, Wave › Tasks) with a filter
that searches that scope. Back follows visits, and Cmd+K routes
cross-repository Work to its own window. Cmd+1–9 and Control+Tab address the same
stable eligible slots, independent of the filter. Each Work owns one retained
workspace. Dashboard queries compose those workspaces side by side; ordinary
navigation selects one destination. Rust owns query membership and arrangement,
including hidden occurrences and duplicate suppression. Tiles show the primary
Session or explicit zoom without acquiring a provider.

## Code map

- `LoopflowMac/Views/RepoView.swift` — one repository/Wave/Task outline and retained workspace
- `LoopflowMac/Views/SessionsView.swift` — every Session in a native split multiplexer
- `Loopflow/Models/MultiplexerLayout.swift` — immutable pane split tree
- `Loopflow/Models/MultiplexerStore.swift` — reference-owned layout, focus, zoom, and undo
- `LoopflowMac/Views/WorkActivityView.swift` — filtered durable Activity and proof links
- `LoopflowMac/WorkModel.swift` — shared readings, stable selection, and local scope
- `LoopflowMac/Views/WavesView.swift` — Portfolio membership and Wave inspectors
- `LoopflowMac/Views/RoadmapView.swift` — all-Wave planning list and lifecycle controls
- `LoopflowMac/Views/WaveDetailPane.swift` — streamed Wave plan, Tasks and metrics
- `LoopflowMac/PortfolioRepoState.swift` — one repository's Wave projection
- `Loopflow/Services/RegistryQuery.swift` — typed `lf --json` reads
- `LoopflowMac/Services/RegistryQueryLocal.swift` — shared Work through the local server; terminal launches and remote file streams through the CLI

The shared `Loopflow` target contains models, queries, and reusable views. The
`LoopflowMac` target contains AppKit, process management, menus, and other
platform behavior. The shared library remains iOS-compatible; there is no iOS
application target.

## Open Source Licenses

Open **About Loopflow → Open Source Licenses** to read and search bundled notices
offline. Matching source and rebuild instructions ship inside the app under
`Contents/Resources/ThirdPartySources`. See [packaging and updates](../third_party/README.md).

## Build system

The launcher builds with Swift Package Manager and uses a stable bundle at
`~/Applications/Loopflow Dev.app` so macOS permissions survive rebuilds.
The launcher clears the calling shell's `LF_*` variables. Dev looks up
`~/.local/bin/lf`, or `$LF_INSTALL_DIR/lf` when the app itself has a custom
installation directory in its environment. Swift edits need only a Swift build.

The installed app updates itself (`AppUpdater`, Sparkle), including a custom
location reached by the public `lf` entry or selected by `LF_APPLICATIONS_DIR`. It
reads its stream from `~/.lf/config.yaml` and runs `lf install adopt` on the first
launch of a new version. Dev builds, and a Machine recording `dev` (which has no
appcast), never check for updates.

| Command | What it does |
| --- | --- |
| `uv run python scripts/loopflow-dev.py run` | Build and open Loopflow Dev on the live Machine |
| `uv run python scripts/loopflow-dev.py install` | Build and install Loopflow Dev without launching |
| `uv run python scripts/loopflow-dev.py run-debug` | Build and open Loopflow Dev with logs visible |
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

The generated app target builds an `lf` helper from
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
lf plan list --all --json --subscribe       # every Wave's plan and open Tasks as Desktop shows them
lf task show LOO-1 --json --subscribe       # the shown Task: work, Graphs, thread
lf session list --json --subscribe         # the repository's Sessions as Desktop lists them
lf repo tracker --json --subscribe       # stored sync receipts
```

A window observes these reads through `RegistryQuery.subscribeWork()`, one server
subscription per part. All `ServedRead` and `ServedWrite` operations cross the
same local protocol, including Task files, diffs, Waves, Machine identity,
Portfolio, saved panes, Task ownership and Graphs. Desktop links no Loopflow Rust
library; GhosttyKit remains its native renderer.

Ghostty owns terminal frame scheduling. Desktop forces a synchronous draw only
for a pending key-to-draw measurement, rather than on every display tick.
Hidden or detached retained surfaces pause rendering while their children and
terminal history continue; showing them resumes the native renderer.

The [server boundary](../docs/dev/server.md) puts registration and updates under
installation. First connection obtains the selected helper; later connections
only reconnect. CLI commands and served writes call the same Rust operations,
which return typed values. Each served mutation runs in a request-scoped worker
with its directory and `CommandIo`; it creates no Process row for the request.
Its response trace correlates diagnostics and effects in `logs/lf.log`.
Task run and ship dispatch detached execution under existing Graph owners.
Terminal launches and remote Task file streams retain their CLI transports;
Desktop's UI automation remains app-owned.

Task changes answer once their write commits; planning sync to Linear runs
after the reply. Each answer carries the store revisions it moved, and each
reading carries the revisions it read at. `PendingChanges` patches shown rows
(the table in [Changes show at once](../docs/surfaces.md#changes-show-at-once))
until a reading at or past the answer's revisions arrives; it drops a patch on a
refusal, never by comparing values or on a timer. Each change carries a request
ID; a repeat within a day returns the stored answer without running again.
`PendingChanges` is one per Machine (Desktop's windows share one), sends each
change under its request ID, resends while the link is lost and holds the link
state: a blip keeps changes in memory, away greys out change controls and
stands one condition Notice. Readings are `Reading<Value>`, carrying `readAt`.
Task creation (including Session adoption) and new Session threads additionally
have creation receipts. `CreationSubmissions` saves the exact submitted payload,
request ID, original Machine and protocol version before sending; the payload
contains the original repository directory. A single app recovery owner looks up
receipts after restart and continues absent or safely resumable submissions.
If an exact New Task replay is refused and the Machine confirms receipt absence,
Desktop parks the submission. **Edit draft** opens it in the saved repository.
Resubmission atomically replaces that journal entry under a fresh request ID.
The server derives `complete`, `continue` or `inspect`; Desktop does not infer
continuation from milestones. `CreationPresentation` retires a submission when
its Task or Session view mounts, including promotion. Cold recovery's Show result
uses the same completion owner after displaying the resource; Inspect retains
unconfirmed launches or conversation changes. Failed removal repeats lookup and
presentation. No local milestone ledger is stored.
An earlier New Task reply leaves later navigation and a newer New Task sheet
alone. Its result stays registered for recovery until presented; only the
originating sheet can close, and opening the result requires unchanged navigation.
See the [creation contract](../rust/loopflow/src/server/README.md#creation-receipts).

The parts: the plan, the repository's Dashboard plans
(`lf desktop view list`), Sessions (`lf session list`) and planning status, the shown Task's status, its
activity (`lf session history`). Each subscription is
internally consistent; panes do not share one snapshot. Changing what the window
shows restarts only the subscriptions whose read changed; the rest answer from their last frame without reading again.

A part arrives again only when a commit changed what it shows, so a Task created
by `lf`, a worker or another window appears without a refresh. A file written or
a commit made in a Task checkout shows within a few seconds. Sessions and the
shown Task are also read on a two-second clock, which is how a quiet Session
turns **Waiting**. Stored Workflow definitions follow the planning revision every
frame carries. File definitions are reread when the app becomes active or after
customization.

After Task actions and Workflow changes, the window waits for a checkpoint: each
running subscription answers the request with a reading begun after the command, and
the checkpoint follows once all have, even when nothing visibly changed. Session
commands confirm only the Sessions subscription. Explicit Retry and recovery after sleep
ask every subscription to read again. If a subscription ends, the window reopens all of them;
the last readings stay on screen marked unavailable until new ones arrive. If one
part keeps failing, automatic retries wait progressively longer, even while other
parts send updates. The retry delay resets only after the observation stays healthy
for about 30 seconds. Every frame names its Loopflow home.

Task actions distinguish the running command from confirmation of its updated
reading. Confirmation expires after ten seconds; an accepted command is never
automatically repeated.

Local subscriptions share a server connection off the main actor. Cancellation
ends only that watch; connection loss resumes its retained reading and queued
frames. See [Reconnect a watch](../rust/loopflow/src/server/README.md#reconnect-a-watch)
for checkpoint and recovery behavior.
Remote Task file subscriptions retain their CLI pipes, drained off the main actor;
cancellation closes stdin, then terminates and reaps only the owned reader if
necessary. A subscription that speaks only on change is not stalled merely by silence.

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
Both surfaces show “Preparing Project…” while that Wave's planning command is pending,
retaining cached planning. Retry appears after failure. An unfinished persisted
Process without a pending local command remains an unknown outcome.
The Work stream supplies `project_readiness`: the selected plan, dated accepted
facts, unresolved transition and the exact activation Process's outcome. Empty default
Graphs render no template section. Command transport errors remain scoped to the Wave.

Work observation and navigation project durable Waves, their planning and Tasks with
associated Sessions and Processes; they introduce no additional Work kinds.
`RepoView` shows one selected repository; `WorkModel` also serves Portfolio.
`TaskProjection` and `WaveProjection` join planning with associated conversations.
`TaskWorktreeSnapshot` describes a Task's Git worktree. `WorkspaceIdentity` and
`SessionWorkspace` retain their names because primary Sessions can use a non-Git
directory. Terminal workspaces own pane layouts. Saved cache paths, selection keys
and placement JSON fields retain their existing bytes across these source renames.

Definition search keeps Graph and Workflow destinations separate, even for the
same name. Planning selectors list only Workflows. Task reads expose
`graph_name`, `latest_graph_process`, `execution`, and `run_control` separately;
completed Graph processes retain their graph and identity. The Task subscription part
supplies `graph_processes`, using the same execution details as CLI inspection.

### Inspect retained workspaces

```sh
lf open loopflow://loopflow/task/LOO-427 --explain --json
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
window. After explicit `config repo --bind`, the next open reuses the existing
scene, including one still registering. Retained local IDs come from `lf`, not
path matching. Drafts, native views and pending links stay in place; old pane
targets become stale, so inspect again before control. Restoration accepts a
retained ID but refuses a location that no longer owns it. Launch acceptance returns `opening`; it does
not prove a usable native endpoint. Inspection retains the latest request's outcome:
`usable` for a Session request requires the Session's native-surface callback, with its
pane still visible. Closing, hiding, replacing or zooming away that pane fails the pending
request, as do connection and surface errors. Navigation cancels an unfinished opening; late results cannot settle a newer
request, even for the same URL. This is endpoint readiness, not provider-turn completion
or proof of compositor rendering. A plain Task link waits for its Task page to mount;
it does not require a Session or checkout. An unavailable workspace reports failure.

Top-level `openings` records the latest repository-opening request per local locator,
including lookup/validation failures before any window registers. A plain repository
becomes usable when its validated workspace receiver registers, even while planning
loads. Registered Task links report their page/Session outcome on the window instead.
Each receipt has a revision so a repeated request cannot consume its previous
result. Session panes report attachment separately from native surface existence.
Failed panes can reconnect in
place, while a pending attachment stays with its current viewer.
These are transient receipts, not opening history or Work identity. Remote Task links
retain the local repository window and re-observe the named execution Machine before
opening its Session. A remote link without `--session` shows the Task without
creating or connecting a Session. Missing, changed or unavailable recorded locations
never prepare another checkout. The CLI transports native Session argv over SSH,
keeping the owner's data directory and takeover behavior. Local panes and
drafts remain retained; no remote planning tree or execution record is imported.

Every Desktop gesture is a `DesktopAction` in `LoopflowMac/DesktopActions.swift`:
name (the `lf desktop` verb), title, arguments, default keys and `perform`.
`DesktopActionMonitor`, the View and File menus, the Keyboard Shortcuts window and
the Desktop socket look actions up there; nothing else mutates navigation or panes.
Keys and menus act on the shown workspace's focused pane; a socket request names an
exact window and pane, and `lf desktop` resolves those from one inspection (see the
[CLI examples](../docs/surfaces.md#script-desktop)).

`DesktopServer` (`LoopflowMac/DesktopServer.swift`) binds
`$LF_HOME/run/desktop/<bundle-id>.sock` at launch (0700 directory, 0600 socket,
same-uid peers, a stale file unlinked) and answers newline-delimited JSON:
`inspect`, `perform`, `read_text` and `events`. It serves only `lf desktop` and
hosts no Sessions. This app-owned automation endpoint closes with Desktop; the
shared client API server has an independent lifetime and serves Work/terminal
access. An unknown action or bad argument throws `DesktopArgumentError`
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

Repository and Wave navigation displays retained conversation entry without
starting a provider.


## Testing

Test shared Work through the [server protocol](../rust/loopflow/src/server/README.md).
Rust's `served_reads` verifies CLI/server mutation parity, input/output/errors,
concurrent request traces in `logs/lf.log`, and no request Process rows or global
context changes. Native cancellation releases the client's wait; a lost reply
remains unknown without replay.

Terminal recovery must preserve the exact owner and draft.
Opening shows **Opening… Input paused** through attachment and native readiness;
**Reconnecting…** follows a previously acknowledged connection.
Connection feedback overlays the terminal, so clearing it does not resize the
provider or move its cursor. Both mount layout and pre-mount sizing keep the same
viewport while status changes.
Refresh waits for its previous viewer to retire before preparing a replacement.
`WarmPaneMountTests` checks this headless with paused input and overlapping
Refresh requests. Opening again replaces an ended attachment; the transport fixture `desktop_crash_reopen` checks that the draft survives
a Desktop crash and new input submits once, against a surviving provider.
`GhosttyTerminalMountTests` checks mounted
focus separately.
Check native status, paused input, local copy/Find/scroll, Retry after exhaustion,
typing from two views at once, and cancellation on pane closure. An orderly terminal close
ends recovery and shows its replacement or retirement reason; bare EOF keeps the
reason unknown. Installed-helper replacement must preserve the Session and Graph
identities and prove continued input and Graph progress; a socket restart alone is insufficient.
`scripts/test_desktop.sh --filter ServerProtocolTests` builds the consumer and
checks shared Rust/Swift fixtures. Installation needs the selected app bundle;
source experiments select a separate `LF_HOME`.

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
CI passes `--skip-build` after its separate Rust and Swift builds so compilation
does not consume the test deadline. With `--skip-build` the script runs the
tests as last built: after an edit, run `swift build --build-tests
--package-path swift` first, or a new test is silently absent and an old
binary passes.

On a pull request CI runs two complementary groups after the build: transport/CLI
fixtures and the remaining model/view tests. Each gets six minutes; main keeps a
20-minute hang guard per group. Both remain under the required `swift-test` check.
The same suite expression supplies `--filter` and `--skip`, so every test runs once.
The local command runs both groups together. A PR that makes a group slower fails
on its own PR: make the slow test fast or share its setup.

After the tests, `swift-test` launches the debug app it built and waits for it
to quit, the same check the release package runs:

```bash
uv run python scripts/check_desktop_launch.py   # launch, snapshot, quit within 30 s
```

It runs `LoopflowMac -ui-test-mode empty-workspaces` with the snapshot
environment, unattended like the rest of the job. Passing means the app wrote a
non-empty snapshot and exited 0. If the app is still running after 30 s, the check
samples it, kills its process group and fails with the app's output and the
sampled stacks, so a hang at launch or quit fails on its own PR
instead of in `Packages (main)` after merge. Read the stack first: it names the
frame the app is waiting in (for #1811's hang, `uiTestSnapshot`).

Failed Swift CI runs write the unfinished and slowest tests to the job summary
(`scripts/swift_test_report.py`) and retain a `swift-tests-<run>-<attempt>`
artifact for seven days with the toolchain and each group's output log and Swift
Testing event stream. Tests run serially within each group. SwiftPM buffers
console output, so silence alone does not establish that test execution never began.

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

A Dashboard is headed by its name, with its query beside it. Each tile is
headed by the Task's row, the same one the sidebar shows, with open in full,
more and hide at its end. Under the row is one sentence for the workflow,
unless the Task's program is asking something, which the row already prints:

```text
Available and blocked   state:available or state:blocked        Default Slots  Edit Dashboard

Keep interrupted terminal pastes from swallowing Session input
LOO-912  Busy
build is running. You are next needed at human-review.
```

Past one page of tiles, the pager reads "Page 1 of 2".

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

Each Mac saves its own pane layouts, selected shell/file tabs and file drafts, one
file per Work under `$LF_HOME/desktop-cache/pane-layouts/<machine>/<kind>/`; two
Macs driving one Machine keep separate layouts. Restoring into open Work keeps its
newer tabs and edits, and a pane naming a Session the current reading lacks opens
empty. A failed save leaves local content intact and shows an error; **Reload saved
intent** reconciles retained content before another save. File drafts compare their saved base with disk before editing. Restored shell
intent retains its identity without initial commands: opening it attaches to the
same owner and never replays startup or creates replacement execution.

Task shell views attach to a retained owner on the Task's recorded Machine.
Desktop supplies its existing Ghostty startup script and resource bytes when
creating the shell; destination creation installs those resources, while reconnect
only attaches. Desktop owns the shell runtime and execution receipts; shared transport has no Ghostty dependency.
Its private destination runtime works without the GUI, including on Linux.
Hide, switching Work and app exit detach presentation. Close ends that shell and
its ordinary jobs; Undo restores its identity and history. Use Reconnect for
transport loss and New Shell for fresh execution. No remote failure opens a
local prompt.

Keep a Task Session viewer open to retain its worktree. Desktop shells accompany
Task Sessions and add no separate hold. Switching Work, closing the window or
quitting explicitly releases Desktop's viewers while execution stays on its
owner. With no connected viewer, a completed Task's conversations, checkout and
Desktop shells become eligible for cleanup after fifteen minutes. The installed
cleanup schedule runs once a minute, so deletion normally starts in 15–16 minutes
while awake. Failed cleanup retries with backoff; a pending shell close continues
on a later pass.

Requested active work, pending requests and running Graphs delay cleanup without
restarting that clock. Unsent drafts do not delay cleanup. LF's remote connection
detects silent loss with viewer heartbeats. Reconnecting before cleanup cancels
it; a longer outage can leave a completed Task without its checkout. Failed Quit
delivery follows that same bounded absence policy.
Conversation history remains; a deleted checkout is not recreated. See the
[shared transport contract](../rust/loopflow-execution/src/transport/README.md#task-shells-and-completed-workspaces).

## iOS client

```sh
xcodegen generate --spec swift/ios.yml
open swift/LoopflowMobile.xcodeproj
```

Run the **LoopflowiOS** scheme and connect a Machine over host Tailscale.
[Loopflow for iOS](LoopflowiOS/README.md) covers its build, screens, terminal
and tests. It draws with the shared tokens and components in
`Loopflow/Design/`, on the iPhone sizes of the ramp in
[DESIGN.md](../DESIGN.md#type).
