# Loopflow for macOS

```bash
./dev run          # build and launch ~/Applications/Loopflow Dev.app
./dev run-debug    # launch with logs visible
./dev test         # run Swift tests
./dev xcode        # regenerate and open the Xcode project
```

Loopflow opens on a repository rail, a Wave list, and the machine-wide roadmap.
The roadmap reads `lf roadmap --json` once, showing every Wave's durable
Projects and Tasks even when its process is stopped. Select **All Repos** for
the whole machine or a repository to filter that same snapshot locally.

Create a Wave with the `+` button. The app writes `GOAL.md` and `MEMORY.md`
in the repository's main checkout. **Open chat** attaches to a live Wave;
**Start & open** launches a stopped Wave through `lf wave` and opens its
conversation while it connects. Wave Chat paints the bounded local journal
tail before SSE, keeps it visible through reconnects, and rolls equivalent
operational failures into one disclosed notice.
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

Open **Go → Context Lab** to filter a local session set, compare its aggregate
context flame and prompt-ordered session lanes, inspect exact trace evidence,
and launch a fresh refinement session in an existing Intelligence Task
worktree. Project and Task facets appear only for durably attributed launches;
historical gaps stay unattributed. Selecting a segment never opens prompt bodies;
**Open trace** is the explicit boundary. Saved views retain only the query and visualization mode.
Revision comparisons stay unavailable until both revisions have enough
similarly captured launches.

## Product ownership

- **Wave Chat** owns the human conversation, the active Wave turn, and
  send/steer/interrupt behavior.
- **Projects and Tasks** appear in the Wave work map. Linear owns their planning
  identity; Loopflow's registry owns their runtime state.
- **Task Sessions** own implementation worktrees and PR delivery. Every Task
  reports through its Project Session; the Wave retains root inspection and
  override. Waves and Projects remain control-plane processes in main.
- **Task workspace presentation** reads `lf task changes/diff/file --json`,
  attaches through `lf task attach`, and keys terminal tabs by Task Session id.
  Lifecycle mutations remain `lf task run/resume/interrupt`.
- **Registry queries** own durable reads. `RegistryQuery` runs
  `lf ls/status/roadmap/runs/usage/doctor/tokens/context/trace --json`; the app
  does not maintain a second roadmap or lifecycle database. Unavailable per-Wave
  evidence renders its reason, and refresh failures leave the last successful
  roadmap or selected Wave detail visible.
- **Per-Wave SSE** owns live motion. `WaveChatConnection` first reads
  `lf chat --history --json`, then connects only to the selected Wave's
  `/events` stream and upserts its replay before continuing live.

## Code map

- `LoopflowMac/Views/WavesView.swift` — repository rail and Wave selection
- `LoopflowMac/Views/RoadmapView.swift` — all-Wave roadmap and lifecycle controls
- `LoopflowMac/Views/WaveDetailPane.swift` — Wave Chat plus Project/Task work
- `LoopflowMac/Views/TaskWorkspaceView.swift` — Task diff, file, Ghostty, and Warp surface
- `LoopflowMac/Views/ContextLabView.swift` — session-set filters, flames, lanes, and evidence
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
