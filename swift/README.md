# Loopflow for macOS

```bash
./dev run          # build and launch ~/Applications/Loopflow Dev.app
./dev run-debug    # launch with logs visible
./dev test         # run Swift tests
./dev xcode        # regenerate and open the Xcode project
```

Loopflow opens on a repository rail, a Wave list, and the selected Wave's live
conversation. It scans main git repositories under `~/src`, remembers added
repositories, and merges authored `wave/<name>/GOAL.md` definitions with the
machine registry.

Create a Wave with the `+` button. The app writes `GOAL.md` and `MEMORY.md`
in the repository's main checkout. Selecting a Wave opens Wave Chat; sending a
message launches or reconnects to `lf wave`, then streams that Wave's turns.
Wave Chat opens on the recent conversation. Commands, tools, file edits, and
loop bookkeeping stay in the journal; decisions, deliveries, and human-level
failures remain visible. The detail pane reads Projects, Tasks, decisions, PR
delivery, and attention from `lf status --json`.

Select a Task, then open its workspace to review changed files, per-file
patches, or current contents. The same sheet keeps multiple embedded
Ghostty/tmux shells for that Task and can open its worktree in Warp.

Open **Go → Telemetry** for token spend, codebase growth, a token-weighted
codebase tree, and registry health.

## Product ownership

- **Wave Chat** owns the human conversation, the active Wave turn, and
  send/steer/interrupt behavior.
- **Projects and Tasks** appear in the Wave work map. Linear owns their planning
  identity; Loopflow's registry owns their runtime state.
- **Task Sessions** own implementation worktrees and PR delivery. Every Task
  reports through its Project Session; the Wave retains root inspection and
  override. Waves and Projects remain control-plane processes in main.
- **Task workspace presentation** reads `lf task changes/diff/file --json` and
  keys terminal tabs by Task Session id. It owns neither Git comparison rules
  nor Task lifecycle.
- **Registry queries** own durable reads. `RegistryQuery` runs
  `lf ls/status/runs/usage/doctor/tokens --json`; the app does not maintain a
  second roadmap or lifecycle database.
- **Per-Wave SSE** owns live motion. `WaveChatConnection` connects only to the
  selected Wave's `/events` stream; human subscriptions replay the latest 12
  turns before continuing live.

## Code map

- `LoopflowMac/Views/WavesView.swift` — repository rail and Wave selection
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
