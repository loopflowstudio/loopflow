# Loopflow for macOS

```bash
uv run python scripts/loopflow-dev.py run          # build and launch
uv run python scripts/loopflow-dev.py install      # install without launching
uv run python scripts/loopflow-dev.py run-debug    # launch with logs visible
uv run python scripts/loopflow-dev.py test         # run Swift tests
```

The development app follows the machine-selected Loopflow Home through the
stable install gate, so launching it from Finder shows the same Waves and
Sessions as terminal `lf`.
Repository selectors list only Git main checkouts. Linked worktrees stay visible
only through the Task Work that owns them.

```text
⌘D          split right
⌘⇧D         split down
⌘⌥←/→/↑/↓   focus the visual neighbor
⌘⇧Return    zoom the focused pane
⌘W / ⌘Z     close / restore a pane
```

Shell panes with Ghostty shell integration group each completed command and its
output into a full-width block. Click anywhere in a block to select the whole
unit, then use Command-C or the terminal's context menu to copy the command and
all of its output.
Command-Up/Down navigates between prompts. The live prompt remains ungrouped;
Session/provider panes keep their native TUI behavior and do not expose shell
command blocks.
Automatic integration depends on the configured shell; macOS `/bin/bash` is
excluded by the pinned Ghostty build.

If macOS cannot provide Ghostty's display link, terminals use timer rendering.
The app logs the CoreVideo error code; the user's Ghostty configuration is unchanged.

Open a repository to see one compact Work list, grouped by collapsible Wave
and Project headings. Incomplete Tasks remain visible whether they are upcoming,
running autonomously, or have human Sessions. Select a Task to inspect its
directive, recorded condition, Project definition and KR proof, Activity, PR,
and worktree references. A Task with no open Sessions says so explicitly.

Open a Session from its subject's terminal button. Sessions absent from the
planning read remain reachable under **Other open Sessions**; existing multiple
conversations and required human decisions stay available. Task joins use the
shared durable Work relationship, never titles or checkout guesses.

Work opens full-width. **Show work list** adds the compact navigator alongside
it; **Hide work list** restores full width. **All work** returns to the overview,
and **Return to terminals** restores the retained pane layout. List visibility,
search, list scroll position, group expansion, and Work selection survive
repository switches for this window. Searching temporarily reveals matching groups without changing
saved expansion. Navigation never starts a provider or resolves a Session.

Each pane owns one native libghostty surface. Session badges distinguish
**VIEWING**, **RUNNING**, **ELSEWHERE**, **OPENING**, and **RETRY**. Sessions
include interactive provider Runs, Task human FlowSteps, and ad-hoc Asks.

Selecting an ELSEWHERE row opens a pane that explains the situation; nothing is
stopped until its explicit **Move here**, which stops the other client and
resumes the Session in that pane. Unsent text typed in the other client is
lost, and the pane says so before you commit.

The green **Complete** action stops an interactive provider client and removes
its Session from the queue while retaining provider-native history. Undo does
not restore a completed Session's pane, even if you hid it before completion. Closing a
pane only hides the view: the terminal and its provider client keep running
(the row shows RUNNING) and reopen exactly as left. An Ask agent
can mark itself ready, but the row and terminal remain until the user completes
the conversation. Task FlowSteps instead expose Approve and Iterate. Closing
or detaching either review boundary never resolves it.

Task FlowSteps run ordinary `lf --tui --as task:<id> <skill>` provider Runs.
Ad-hoc Asks run in the originating Run's exact checkout so the session can edit
files before the caller resumes. The app lists, opens, and acts on the shared
Rust `SessionRecord` projection; it owns no parallel queue.
Use **New conversation** to talk about the selected repo, Wave, Project, or Task
in the configured app or terminal. It opens an interactive prompt without
creating a Task or running an autonomous operating pass. **New terminal** opens
an ordinary shell in the active checkout. A conversation launched here returns
to a shell when it exits or hands off to an external app.

Selecting a Session in another checkout restores that worktree's conversation,
companion terminals, split layout, and focus. Worktree headers split entire
workspaces; terminal headers split within one workspace. Hiding a worktree
retains its processes. Closing a shell ends that shell. Changing a shell's
directory does not move it into another workspace.

Manually launched agents in these shells register against their actual terminal.
Selecting their Session focuses the existing shell. External clients still read
ELSEWHERE and require explicit Move here. Terminals survive Work list and detail
navigation and repository switches within a window. Native surfaces belong to
that window and are never mounted twice.
Session reads and preparation run in the opened repository rather than a
machine-wide aggregate.

Planning and Sessions share the Podium readings; there is no separate
Sessions-only roadmap query. The displayed planning timestamp is snapshot
generation time, not evidence of a fresh provider sync. A failed read keeps its
last useful evidence and exposes the error. Unknown Session counts show **?**.

Current Wave navigation reads `lf ls --all --current --json`. The CLI excludes
abandoned and retired registrations; unfiltered `lf ls` retains historical
registry visibility. Authored `wave/<name>/GOAL.md` files still appear before
their first registration.


The compact Podium bar reads live process evidence from `lf ps --json`. Its
lamp reflects OS-live state: black is off, green is working, blue is stalled,
and amber is waiting or unknown. Wave count, active Runs, and
Run-without-listener warnings come from `lf ls --json`.
Its Sessions badge uses the same repo-scoped `lf session list --json` reading
and reveals the work list.
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
reads Projects, Tasks, decisions, PR delivery, and Task conditions from `lf
status --json`.

Start, resume, attach, or interrupt a Task from the roadmap. Open its worktree
in Warp, or attach to the running Task agent in the workspace sheet beside its
changed files, per-file patches, current contents, and embedded shells.
The condition chip and spoken row use the same `lf roadmap` reason: green is a
live advancing body, blue is waiting, red is blocked, black is settled or
unstarted, and unknown means the required evidence could not be read.

Open **Go → Telemetry** for token spend, codebase growth, a token-weighted
codebase tree, and registry health.

## Product ownership

- **Wave Chat** owns the conversation, the active Wave turn, and
  Send and bare Interrupt controls.
- **Projects and Tasks** appear in the Wave work map. Linear owns their planning
  identity; Loopflow's registry owns their runtime state.
- **Tasks** own implementation worktrees and PR delivery. Every Task
  reports through its Project Work; the Wave retains root inspection and
  override. Waves and Projects remain control-plane processes in main.
- **Task workspace presentation** reads `lf task changes/diff/file --json`.
  Lifecycle mutations remain `lf task run/resume/interrupt`; review nodes use
  the Task's persisted flow position and provider Run identity.
- **Registry queries** own durable reads. `RegistryQuery` runs
  `lf ls/status/roadmap/ps/activity/usage/doctor/tokens --json`; the app does not
  maintain a second roadmap or lifecycle database. Unavailable per-Wave evidence
  renders its reason, and refresh failures leave the last successful roadmap or
  Activity history visible.
- **Per-Wave SSE** owns live motion. `WaveChatConnection` first reads
  `lf chat --history --json`, then connects only to the selected Wave's
  `/events` stream and upserts its replay before continuing live.

## Code map

- `LoopflowMac/Views/PodiumView.swift` — primary Wave scope, Work, and live process signal
- `LoopflowMac/Views/SessionsView.swift` — every Session in a native split multiplexer
- `Loopflow/Models/MultiplexerLayout.swift` — immutable pane split tree
- `Loopflow/Models/MultiplexerStore.swift` — reference-owned layout, focus, color, and undo
- `LoopflowMac/Views/WorkActivityView.swift` — filtered durable Activity and proof links
- `LoopflowMac/PodiumModel.swift` — shared readings, stable selection, and local scope
- `LoopflowMac/Views/WavesView.swift` — previous Wave workspace during migration
- `LoopflowMac/Views/RoadmapView.swift` — all-Wave roadmap and lifecycle controls
- `LoopflowMac/Views/WaveDetailPane.swift` — Wave Chat plus Project/Task work
- `LoopflowMac/Views/TaskWorkspaceView.swift` — Task diff, file, Ghostty, and Warp surface
- `LoopflowMac/PortfolioRepoState.swift` — one repository's Wave projection
- `Loopflow/Services/RegistryQuery.swift` — typed `lf --json` reads
- `Loopflow/Services/WaveChatClient.swift` — per-Wave event and message client
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

The dev app bundles the current source `lf` with release Home selection and
validation-only migration authority. Its operator views therefore read the
real Home without allowing an unpromoted build to advance the shared database
frontier. Ordinary source-built `lf` commands keep their isolated `.lf-dev`
Home.

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

The generated app target builds validation-only `lf` and `lfd` helpers from
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
