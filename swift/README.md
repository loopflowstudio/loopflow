# Loopflow for macOS

```bash
./dev run          # build and launch ~/Applications/Loopflow Dev.app
./dev run-debug    # launch with logs visible
./dev test         # run Swift tests
./dev xcode        # regenerate and open the Xcode project
```

Loopflow opens on the control room: repository and Wave scope on the left,
machine-wide Now/Roadmap work in the center, and selected Work evidence on the
right. The strip above them shows active agents, attributed motion,
five-minute output, measured tokens, and orphaned or unclaimed providers from
`lf ps --json`. Beside that activity it keeps Wave count, active Runs,
answering listeners, active Project/Task counts, and Run-without-listener
warnings from `lf ls --json`. Selecting a Wave, Project, or Task preserves the
surrounding live view. The control room reads `lf roadmap --json`,
`lf ls --json`, and `lf ps --json` once per refresh; repository scope filters
the Work and Wave snapshots locally while agent activity remains machine-wide.

Loading, empty, stale-last-good, and unavailable reads stay distinct. Wave and
agent readings fail independently. A failed refresh keeps the last useful
evidence visible with its failure reason rather than painting a healthy empty
fleet.

The previous Wave workspace remains available in repository and Portfolio
windows while its proven inspectors and Chat surface move into the new root.
Wave Chat paints the bounded local journal tail before SSE, keeps it visible
through reconnects, and rolls equivalent operational failures into one
disclosed notice. Cold launch does not start a chat transcript read.
Commands, tools, file edits, and loop bookkeeping stay in the journal;
decisions, deliveries, and human-level failures remain visible. The detail pane
reads Projects, Tasks, decisions, PR delivery, and attention from
`lf status --json`.

Start, resume, attach, or interrupt a Task from the roadmap. Open its worktree
in Warp, or attach to the running Task agent in the workspace sheet beside its
changed files, per-file patches, current contents, and embedded shells.
The attention chip and spoken row use the same `lf roadmap` reason: green is a
live advancing body, red is a human handoff or local recovery, black is settled
or unstarted, and unknown means the required evidence could not be read.

Open **Go → Telemetry** for token spend, codebase growth, a token-weighted
codebase tree, and registry health.

Select a Wave, then open **Context Lab** from its header. Compare that Wave's
aggregate initial-prompt flame and prompt-ordered Invocation lanes, or rank
current instruction sources by captured Invocation impressions. Select a skill or
`LOOPFLOW.md` to read main's current file beside exact trace evidence. Choose a
Refinement Project once per multi-Project Wave, then **Refine in task-worker**
creates a Task and opens its running agent. The Project destination does not
filter the Wave evidence. Repo and Wave stay fixed for the window; saved views
store only filters inside that scope. Historical attribution gaps stay unattributed.
Selecting a segment never opens prompt bodies;
**Open trace** is the explicit boundary. Saved views retain only the query and visualization mode.
The research-state filters can require observed steering or a launch containing
a current resolvable file-backed instruction revision. Revision comparisons stay unavailable until
both revisions have enough invocations with comparable capture, provider/model mix,
and observation spans.

## Product ownership

- **Wave Chat** owns the human conversation, the active Wave turn, and
  send/steer/interrupt behavior.
- **Projects and Tasks** appear in the Wave work map. Linear owns their planning
  identity; Loopflow's registry owns their runtime state.
- **Tasks** own implementation worktrees and PR delivery. Every Task
  reports through its Project Work; the Wave retains root inspection and
  override. Waves and Projects remain control-plane processes in main.
- **Task workspace presentation** reads `lf task changes/diff/file --json`.
  Lifecycle mutations remain `lf task run/resume/interrupt`; routed questions
  are answered through the explicit `lf work asks/answer` CLI.
- **Registry queries** own durable reads. `RegistryQuery` runs
  `lf ls/status/roadmap/ps/runs/usage/doctor/tokens/context/trace --json`; the app
  does not maintain a second roadmap or lifecycle database. Unavailable per-Wave
  evidence renders its reason, and refresh failures leave the last successful
  roadmap or selected Wave detail visible.
- **Per-Wave SSE** owns live motion. `WaveChatConnection` first reads
  `lf chat --history --json`, then connects only to the selected Wave's
  `/events` stream and upserts its replay before continuing live.

## Code map

- `LoopflowMac/Views/ControlRoomView.swift` — primary scope, live Work, and selected detail
- `LoopflowMac/Views/ControlRoomStatusStrip.swift` — Wave fleet health and machine-wide agent motion
- `LoopflowMac/ControlRoomModel.swift` — shared readings, stable selection, and local scope
- `LoopflowMac/Views/WavesView.swift` — previous Wave workspace during migration
- `LoopflowMac/Views/RoadmapView.swift` — all-Wave roadmap and lifecycle controls
- `LoopflowMac/Views/WaveDetailPane.swift` — Wave Chat plus Project/Task work
- `LoopflowMac/Views/TaskWorkspaceView.swift` — Task diff, file, Ghostty, and Warp surface
- `LoopflowMac/Views/WorkCensusView.swift` — machine-wide Work and Invocation census
- `LoopflowMac/Views/ContextLabView.swift` — invocation-set filters, flames, lanes, and evidence
- `LoopflowMac/Views/ContextLabHandoffView.swift` — explicit trace bodies and Task refinement handoff
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

| Command | What it does |
| --- | --- |
| `./dev run` | Build and launch |
| `./dev run-debug` | Build and run with stdout |
| `./dev build` | Build only |
| `./dev test` | Run unit tests |
| `./dev ui-test` | Generate the project and run UI tests |
| `./dev xcode` | Generate and open the project |
| `./dev release` | Build the release app and DMG |
| `./dev clean` | Remove the development app and reset permissions |

Long-running development commands write logs under `~/.lf/logs/dev/`.

`project.yml` generates `LoopflowSwift.xcodeproj`:

```bash
xcodegen generate
xcodebuild -project LoopflowSwift.xcodeproj \
  -scheme LoopflowMac \
  -destination 'platform=macOS' \
  build
```

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
