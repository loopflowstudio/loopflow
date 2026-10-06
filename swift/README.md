# Loopflow for macOS

```bash
uv run python scripts/loopflow-dev.py run          # build and launch
uv run python scripts/loopflow-dev.py install      # install without launching
uv run python scripts/loopflow-dev.py run-debug    # launch with logs visible
uv run python scripts/loopflow-dev.py test         # run Swift tests
```

The development app bundles this checkout's CLI. Ordinary launches use the
installed CLI and main Home. For a branch demo, launch with an explicit private
`LF_HOME`; that uses the matching source CLI on disposable data. A copied Home
retains real checkout paths, so file edits still affect those checkouts.
Repository selectors list only Git main checkouts. Linked worktrees stay visible
only through the Task Work that owns them.

A returning launch opens on the workspace saved at `<Home>/desktop-cache/workspace.json`
and shows **Updating…** until this launch's reads replace it. Saved rows open
and navigate; Flow state, Task condition, Session state and every action except
opening a Session wait for the fresh read. The Portfolio window opens from the
same saved workspace. A failed refresh keeps what is shown under one
**Couldn't update** line. Repeated identical Session errors leave the displayed
reading unchanged; a new error or recovery updates it. Delete the file to start
from **Loading workspace…**.

```sh
uv run python scripts/benchmarks/desktop-performance/timings.py   # how long your launches took
```

Each launch records when its first frame, saved workspace and fresh workspace
arrived, and how long each `lf` read took, under `desktop-cache/timings/` in
the Home. Durations and command names only; the files stay on this machine.

Choose **Background progress** in the repository toolbar to enable minute
checks, enroll or hold Tasks, and inspect the last check and blockers. Checks
continue with the app closed on the selected Home. Disabling stops scheduled
admission; already-running work and requested GitHub merges continue.

While a repository is open, the app runs `lf ci watch` for it and stops it on
quit. The watcher starts a ci-fix when a recorded landing fails its required
checks; `lf ci watch --status` shows its last poll and what it started.

```text
⌘D          split right
⌘⇧D         split down
⌘⌥←/→/↑/↓   focus the visual neighbor
⌘⇧Return    zoom the focused pane
⌘W / ⌘Z     close / restore a pane
```

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

Choose **Show Files** in a Task or Session toolbar to browse its checkout beside
the retained terminal. Expand folders to read one page of up to 500 entries;
**Load more** continues the directory and **Show ignored** includes ignored paths.
Tracked, unchanged and untracked files remain available without an active PR.
**Changes** optionally compares the worktree with the Task PR's
recorded base; **HEAD** compares with its current commit. The displayed SHA pins
the file list and selected diff. A recorded PR links above the file navigator.
Use ↑/↓ in the navigator to select files; hover a row for its full path.
**Show Files** / **Hide Files** remembers the browser visibility across relaunches.

**File** retains editable UTF-8 drafts and selection in this window; **Diff**
shows the draft comparison read-only. Task headers show the recorded checkout;
terminal and files follow that Task without a separate worktree selector.
Switching files, hiding Files and returning to the Task keeps the draft.
Files reached through symlinks remain readable within the checkout and show a
read-only explanation. If a regular file becomes a symlink, its retained draft
and Undo survive while editing, Save and autosave stop.

Typing autosaves after two quiet seconds. Disable **Autosave Task Files** in the
app menu for explicit **Save** (⌘S). Both modes receive filesystem changes live,
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

Select a repository, then a Task. Its workspace retains conversations, shells,
file drafts and layout across visits. Use **+** in the toolbar for a conversation
or shell, and the document icon for Files. The sidebar icon hides or restores the
Sessions pane without changing its terminals or drafts. Click the Task title for
details and the full Flow; the toolbar's current stage opens its available review.

Exiting a disposable CLI conversation retires it from navigation while keeping
its provider history. Ctrl-C interruption is recorded; interrupted Flow reviews,
Task conversations and repository/Wave primaries remain reachable.
Stopping a response alone never closes the conversation or completes a review.

Each Session or shell's **…** menu holds its pane actions. **Collapse** hides a pane while its process
continues; **Expand** returns to it. **Focus** gives one conversation the content
area; **Restore** brings back the panes and files. **Terminate** ends a shell.
New conversations arrive in the list without taking focus.

Task rows and their counts show unfinished interactive Sessions in the Task checkout,
including independent conversations and authored Flow reviews. **Show headless Sessions**
includes background conversations; `lf session list --interactive all --history`
inspects the full history. Filtering preserves retained terminals and drafts.
Repo and Wave Sessions retain their own scopes. Collapsed Wave rows roll up Task
Sessions; choose a name to open it directly.
**Available** means the conversation can be joined; **Ready to complete** means
its agent has recorded a summary. Failed provider starts offer recovery.

Use the outline menu for **Compact**, **Full hierarchy**, or **Sessions**.
Repository conversations and Sessions with unavailable ancestry stay reachable
below the outline. Sessions without a Task association are also available under
**Debug → Sessions → Orphan Sessions** in the repository menu, or `lf session list --orphan`.
There is no creation opt-out from checkout association. **Task details** opens the description, comments and Run history.

The Task's compact Flow shows interactive stages and inspectable background work.
Select a current stage to join its exact conversation; select a future stage to
inspect it. **Detailed Flow** exposes the captured branches and repeats. The header
uses the shared read: before start, the chosen
Flow's preview (click its name to search the catalogue) and **Start**; once
started, the pinned definition, the current occurrence, and each loop's iteration.
Hover or focus the name for **Stop & restart…**, which confirms before replacing
the Flow. Controls are disabled with Rust's reason when they cannot be used.
A pinned Flow without a live worker reads **Stopped** and offers **Resume**;
there is no Pause until Loopflow can hold a Flow at a boundary.
**Session history** under the Flow reads nothing until expanded; it then lists that
Task's complete recorded input history (`lf usage --days 0 --task ID --json`) with each recorded
outcome. Session rows show their recorded provider and a ready summary
only when the Session recorded one.
Below the Description, **Comments (n)** is collapsed and counts the Task's Linear
thread (`lf task comment ID --json`, read when the Task is shown and
again on expanding). A failed read keeps the earlier thread marked **May be out
of date** with **Retry**; comments are never split out of the Description.
Use **Inspect** or the row's context menu for details. Presentation changes never
start a provider or resolve a Session.

The toolbar above the workspace is its only chrome. On the left it drills
Wave → Task → Session: Wave and Task crumbs return to their details; the Task's
issue ID links to Linear. With several Sessions, the final crumb chooses among
them by name. The pencil renames the Session in place through `lf session
rename`; the shown name is the shared readback, and a rejected name stays in the
field with its error. A remote Flow Session's name lives on its Home and is not
renamed here. Beside the name, the Session shows its Flow step and iteration as
quiet mono text, **Independent**, or why membership is unknown. On the right sit
the Task's read-only worktree location, **Complete** for the focused pane's
conversation when its shared actions allow it, and the Activity and Monitor
glyphs.

A single pane has no header. With two or more, each pane carries a 24pt strip:
a state dot and the conversation's name (or the shell's title). Hovering the
focused pane's strip reveals split right, split down and close; the strip's
context menu and the keybinds above offer the same plus zoom. Unfocused panes
dim slightly; nothing draws a focus border.

**Monitor** opens beside retained Sessions and shells in the same multiplexer.
Split, resize, zoom, close and Undo work for all pane content. Monitor shows the
selected Task's active Sessions and updates automatically. **Refresh** requests an
observation from the same reader. Recovery and read failures retain the last
observation with a visible reason; **Retry** restarts a failed reader. Incomplete
ownership evidence cannot report confirmed emptiness. Closing a Monitor preserves
the window's shared reader and terminals. **Sessions** selects an exact conversation and
returns keyboard focus to its terminal with unfinished input retained.

Press **⌘K** to search Waves, all planned Tasks, named Sessions and Flows in the
selected repository. Empty search shows recent destinations first; arrows select,
Return opens and Escape returns focus to the previous terminal. Flow entries
inspect the template. **Choose Flow for ISSUE** opens the Task's existing picker.
The sidebar search remains a filter. Each repository retains up to 20 recent
destinations for the window. Visited historical Tasks remain in recents after
leaving their pages; opening one reads its exact identity again. This does not
search all historical Tasks. Failed reads preserve the workspace and offer Retry.
If a highlighted row disappears, Return opens the first remaining visible result;
with no results it does nothing.

With an unassigned Session selected, choose **Bind to Task…** in ⌘K or beside
the Session name. Both open the same picker: review the named Task, then choose
**Bind permanently**. Opening the picker leaves the conversation and terminal intact.

```sh
open 'loopflow://task/LOO-303'
open 'loopflow://task/LOO-303?repo=%2Fsrc%2Floopflow'
open 'loopflow://task/LOO-303?repo=%2Fsrc%2Floopflow&session=SESSION_ID'
```

Task links and palette Task entries open retained workspaces, including Tasks
outside the current chapter, without starting a Task Flow. A repository-qualified
link opens its exact match even when another Wave's planning is unavailable.
Ambiguous links offer repository-qualified choices; failed reads keep the current
workspace and offer Retry. Only one workspace window receives a link.
Links prefer a window already selecting the destination. Reopening its Task keeps
the selected conversation and pane layout. Loaded repository-qualified links reuse
observed planning; successful opens do not present a Task-finding sheet.
Add `session` to open an existing Task conversation in its terminal pane. A
missing or unrelated Session leaves the current workspace intact and offers Retry.
Opening a conversation does not complete a review or start a Task Flow.

Wave details show the objective, current chapter plan/KRs, the current Project's
Flow template, current Tasks and chapter history. Completed Tasks start hidden.
**Completed** adds the last 7 days. Click **7 Days** to edit the number inline;
Enter or clicking away applies it, Escape cancels. Enter **0** for **All Tasks**
(successful completions of any age); click that label to edit again. The checkbox
hides history and remembers the range. Invalid input keeps the previous range. Canceled and duplicate history stays out of this list. Unresolved
execution and retained Session access remain available. Templates fold composed Flows;
click a group or its disclosure control to expand it. Repeated uses disclose
independently, and both return edges remain visible at folded boundaries. Tab to
a disclosure, then use Right/Left to expand/collapse or Space/Return to toggle;
this includes nested and empty groups and XOR paths. Inspecting a return names
its target step even while the containing Flow is folded.
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
include interactive provider Sessions and Task human FlowSteps.
Runs resumed interactively also appear, including those originally launched
headlessly. Closing their client preserves the Session until Complete.

Selecting a Session active in another client opens a pane that explains the situation; nothing is
stopped until its explicit **Move here**, which stops the other client and
resumes the Session in that pane. Unsent text typed in the other client is
lost, and the pane says so before you commit.

**Complete**, at the toolbar's right end, stops an interactive provider client and removes
its Session from the queue while retaining provider-native history. If completion
is rejected, its error stays visible in the toolbar through refresh and the terminal remains usable;
retry Complete after addressing the error. Undo does
not restore a completed Session's pane, even if you hid it before completion. Closing a
pane only hides the view: the terminal and its provider client keep running
and reopen exactly as left. Flow reviews expose Complete after their agent marks Ready. Complete returns the feedback to the following decision step, which chooses Advance or Iterate.
Rejected completion preserves the terminal and keeps its error visible through refresh.
The shared Session projection supplies action labels, unavailable reasons and Work
paths to both CLI and Mac; local terminal presence only determines which pane to show.
Closing or detaching a review never resolves it.

Task FlowSteps run ordinary `lf --mode tui --task <id> <skill>` provider Sessions.
The app lists, opens, and acts on the shared
Rust `SessionRecord` projection; it owns no parallel queue.
The Session ID targets conversation actions and history lookup. A prepared
input alone does not establish live provider activity.

Choose a skill beside **New Session** below the repository name. Search by name
or description, use ↑/↓ and Return to select, or Escape to close. Selection is
remembered per repository and does not launch anything. Click **New Session**
to launch the selected skill in an interactive conversation.

The default **capture-tasks** skill explores an idea and captures
Tasks for their owning Waves, including in other repositories. From a Task, capture
starts with its parent Wave in the repository checkout. The conversation remains
available for more ideas; filing does not start workers or move existing panes.
Wave rows also offer **New Session** with the selected skill in their context menu.

Use **New conversation** to talk about the selected repo, Wave, or Task
in the configured app or terminal. It opens an interactive prompt without
creating a Task or running an autonomous operating pass. **New shell** opens
an ordinary shell in the active checkout. A conversation launched here returns
to a shell when it exits or hands off to an external app.

In a Task workspace, one existing Session starts with the Sessions sidebar hidden;
multiple Sessions show a collapsible sidebar. Use the toolbar to toggle it. Clicking
a Session focuses its visible pane or opens it in the active Session pane, preserving
other splits. **⌘-click** toggles a pane without ending the Session; **Option-click**
or **Open alongside** reveals its saved split or opens it to the right. Visible rows
are highlighted, and the focused row has an accent marker. New arrivals preserve
the current layout and sidebar preference. Running shells stay intact.

Selecting a Session in another checkout restores that worktree's conversation,
companion terminals, split layout, and focus. Outside Task context, the toolbar's
worktree menu splits entire workspaces; pane strips and keybinds split within one
workspace. Hiding a worktree retains its processes. Closing a shell ends that shell. Changing a shell's
directory does not move it into another workspace.

Manually launched agents in these shells register against their actual terminal.
Selecting their Session focuses the existing shell. **Complete** ends the attached
Session while keeping that shell available; rejected completion stays visible.
External clients require explicit **Move here**. Terminals and their command titles survive Work list and detail
navigation and repository switches within a window. Native surfaces belong to
that window and are never mounted twice.
Session reads and preparation run in the opened repository rather than a
machine-wide aggregate.

Planning and Sessions share the Podium readings; there is no separate
Sessions-only roadmap query. A failed read keeps its last useful evidence and
exposes the error. Inspection shows the planning snapshot's generation time,
which does not establish a fresh provider sync, and an explicit no-Session state
only after a successful Session read. Compact retains an empty Wave
as an inspectable leaf; it compresses structural levels only when descendants
can take their place.

Current Wave navigation reads `lf wave list --all --current --json`. The CLI excludes
abandoned and retired registrations; unfiltered `lf wave list` retains historical
registry visibility. Authored `wave/<name>/GOAL.md` files still appear before
their first registration.

Repository scope filters the Work and Wave snapshots locally; live process
evidence remains machine-wide.
Provider activity remains separate from Task condition and human Session presence.

Loading, empty, stale-last-good, and unavailable reads stay distinct. Wave and
agent readings fail independently. A failed refresh keeps the last useful
evidence visible with its failure reason rather than painting a healthy empty
state.

The previous Wave workspace remains available in repository and Portfolio
windows while its proven inspectors and Chat surface move into the new root.
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
from `lf wave status <wave> --json`.

Start, resume, attach, or interrupt a Task from the roadmap. Open its worktree
in Warp, or attach to the running Task agent in the workspace sheet beside its
changed files, per-file patches, current contents, and embedded shells.
The condition chip and spoken row use the same `lf roadmap` reason: green is a
live advancing body, blue is waiting, red is blocked, black is settled or
unstarted, and unknown means the required evidence could not be read.

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
  Lifecycle mutations remain `lf flow start` and `lf interrupt`; review nodes use
  the Task's persisted flow position and provider Run identity.
- **Registry queries** own durable reads. `RegistryQuery` runs
  `lf wave list/status/roadmap/ps/activity/usage/doctor/tokens --json`; the app does not
  maintain a second roadmap or lifecycle database. Unavailable per-Wave evidence
  renders its reason, and refresh failures leave the last successful roadmap or
  Activity history visible.


## Code map

- `LoopflowMac/Views/PodiumView.swift` — one repository/Wave/Task/Session outline and retained workspace
- `LoopflowMac/Views/SessionsView.swift` — every Session in a native split multiplexer
- `Loopflow/Models/MultiplexerLayout.swift` — immutable pane split tree
- `Loopflow/Models/MultiplexerStore.swift` — reference-owned layout, focus, zoom, and undo
- `LoopflowMac/Views/WorkActivityView.swift` — filtered durable Activity and proof links
- `LoopflowMac/PodiumModel.swift` — shared readings, stable selection, and local scope
- `LoopflowMac/Views/WavesView.swift` — previous Wave workspace during migration
- `LoopflowMac/Views/RoadmapView.swift` — all-Wave roadmap and lifecycle controls
- `LoopflowMac/Views/WaveDetailPane.swift` — current chapter plan, Tasks and Runs
- `LoopflowMac/PortfolioRepoState.swift` — one repository's Wave projection
- `Loopflow/Services/RegistryQuery.swift` — typed `lf --json` reads
- `LoopflowMac/Services/RegistryQueryLocal.swift` — local `lf` subprocess

The shared `Loopflow` target contains models, queries, and reusable views. The
`LoopflowMac` target contains AppKit, process management, menus, and other
platform behavior. The shared library remains iOS-compatible; there is no iOS
application target.

## Build system

`./dev` uses Swift Package Manager and installs to a stable application path
so macOS permissions survive rebuilds. The app queries `lf` directly and starts
only the selected Wave's `lf wave` process; it has no machine-wide service or
remote-connection mode.

The dev app uses the installed CLI and main `~/.lf` Home. Its bundled source
CLI forwards ordinary commands to that installation. Explicit `LF_HOME`
experiments remain disposable and never become the app's default.

For a branch UI demo, select a private `LF_HOME` explicitly so the bundled CLI
and UI use this checkout’s wire protocol. Its recorded checkout paths still
refer to real files. A copied Home does not prove live provider continuation.

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

Task Monitor's shared reader is `RegistryQuery.watchActiveSessions()`, backed by
`lf monitor active --watch --json`. Its `sessions` rows retain stable AgentSession IDs, titles and current typed Work.
Verified Exec/process and native-client receipts establish activity independently
of command outcomes. Input replacement keeps the same row; unresolved engine
ownership stays a gap. SQL ownership is reread on every tick, even outside Home. Confirm emptiness
only when `discovery` is `ready` and `gaps` is empty. Keep scanning, unavailable,
and incomplete evidence visible. Podium starts one reader on first demand and
retains it across pane and repository navigation until window teardown. Wake
requests a rescan. Helper/Home configuration replacement drains the old reader
and clears its evidence before starting the new one. Pipes drain off the main
actor, frames are limited to 16 MiB, and pending delivery retains only the latest
snapshot. Ten seconds without a frame pauses updates until Retry. Cancellation
closes stdin, then terminates and reaps only the owned reader if necessary.

## Headless checks

```bash
scripts/test_desktop.sh --filter DesktopHeadlessTests
uv run python scripts/test.py --swift
```

Gate and CI build the app and inspect production views and controls without
launching a window. Display/terminal integration is opt-in with
`LOOPFLOW_NATIVE_TESTS=1`; use it for configured-host diagnostics or demo.
See [TESTING.md](../TESTING.md) for the full commands and CLI prerequisite.
