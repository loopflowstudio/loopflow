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

Opening Loopflow to a repository leads with its Sessions queue. Select a session
to resume its provider-native terminal in the focused Ghostty pane; selecting it again
jumps back to that pane. Each pane owns one native libghostty surface. Sessions
include native interactive provider Runs, Task human FlowSteps, and `lf ask`
calls made by ordinary Runs. Rows use the real prompt or Task title plus the
actual provider, Skill, or Run detail.

The green **Complete** action stops an interactive provider client and removes
its Session from the queue while retaining provider-native history. Closing its
pane stops only the current client; the Session stays resumable. An Ask agent
can mark itself ready, but the row and terminal remain until the human completes
the conversation. Task FlowSteps instead expose Approve and Iterate. Closing
or detaching either human boundary never resolves it.

Task FlowSteps run ordinary `lf --tui --as task:<id> <skill>` provider Runs.
Ad-hoc Asks run in the originating Run's exact checkout so the session can edit
files before the caller resumes. A thin detached PTY cradle keeps only the
initial client alive; selecting a Session stops that client and resumes its
provider-native history. The app lists, opens, and acts on the shared Rust
`SessionRecord` projection; it owns no parallel queue.
Selecting another session replaces the focused pane. Use the split controls
first when both sessions should remain visible; closing the final pane clears it
without ending the durable session.
Completing the selected Session returns the main pane to its empty workspace.
Use **New shell** there or in the sidebar for a bare terminal, and **Waves &
roadmap** to return to Work.
Session reads and preparation run in the opened repository rather than a
machine-wide aggregate.

The Podium keeps a closable Wave hierarchy above the Sessions and Work surfaces.
Its Work surface shows machine-wide Now/Roadmap Work beside durable Activity. Selecting a
Wave, Project, or Task preserves the live view and scopes `lf activity --json`
at the source. Disclose each branch from Wave → Project → Task → live Exec.
Exec remains process evidence rather than a fourth Work kind. PR facts open
GitHub proof.

The compact Podium bar reads live process evidence from `lf ps --json`. Its
lamp reflects OS-live state: black is off, green is working, blue is stalled,
and amber is waiting or unknown. Wave count, active Runs, and
Run-without-listener warnings come from `lf ls --json`.
Its Sessions badge reads repo-scoped `lf session list --json` and returns to
the Sessions screen.
Repository scope filters the Work and Wave snapshots locally; live process
evidence remains machine-wide.
Each provider node retains its existing repository and Work attribution, so the
hierarchy rolls one process reading up to Task, Project, and Wave without another
telemetry store. Authored Waves count even before they have an active Run.

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
decisions, deliveries, and human-level failures remain visible. The detail pane
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

- **Wave Chat** owns the human conversation, the active Wave turn, and
  send/steer/interrupt behavior.
- **Projects and Tasks** appear in the Wave work map. Linear owns their planning
  identity; Loopflow's registry owns their runtime state.
- **Tasks** own implementation worktrees and PR delivery. Every Task
  reports through its Project Work; the Wave retains root inspection and
  override. Waves and Projects remain control-plane processes in main.
- **Task workspace presentation** reads `lf task changes/diff/file --json`.
  Lifecycle mutations remain `lf task run/resume/interrupt`; human nodes use
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
| `uv run python scripts/loopflow-dev.py test` | Run unit tests |
| `uv run python scripts/loopflow-dev.py xcode` | Generate and open the Xcode project |
| `uv run python scripts/loopflow-dev.py release` | Build the release app and DMG |
| `uv run python scripts/loopflow-dev.py clean` | Remove the development app and reset permissions |

Long-running development commands write logs under `~/.lf/logs/dev/`.

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
