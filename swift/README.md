# Concerto — macOS App

Visual interface for loopflow. SwiftUI, requires macOS 15+.

## Quick Start

```bash
# Build and run (recommended for development)
./dev run

# Build and run with logs visible
./dev run-debug

# Run tests
./dev test

# Open in Xcode
./dev xcode
```

## Build System

The `./dev` script is the primary build tool. It uses Swift Package Manager (`Package.swift`) for building and installs to `~/Applications/Concerto Dev.app` to preserve macOS permissions across rebuilds.

### Commands

| Command | Description |
|---------|-------------|
| `./dev run` | Build and launch the app |
| `./dev run-debug` | Build and run with stdout visible |
| `./dev build` | Build only |
| `./dev test` | Run tests |
| `./dev ui-test` | Generate Xcode project and run UI tests |
| `./dev xcode` | Open in Xcode |
| `./dev release` | Build release .app and .dmg |
| `./dev clean` | Remove dev app and reset permissions |

### Xcode Project

The `project.yml` (XcodeGen) generates `LoopflowSwift.xcodeproj` for Xcode development and UI tests. Both build systems (SPM and XcodeGen) should stay in sync.

```bash
# Regenerate Xcode project
xcodegen generate

# Build with Xcode
xcodebuild -scheme Concerto -destination 'platform=macOS' build
```

## Embedded Terminal (Ghostty)

Concerto embeds Ghostty for terminal functionality.

### Building Ghostty

Ghostty must be built before the terminal works:

```bash
# Install Zig if needed
brew install zig

# Build Ghostty xcframework
./scripts/build-ghostty.sh

# Or manually:
cd vendor/ghostty && zig build -Doptimize=ReleaseFast
```

See `Concerto/Services/Ghostty/README.md` for integration details.

## Architecture

- `AppState.swift` — Central observable state
- `Services/` — Data loading, no UI
- `Views/` — SwiftUI views
- `Models/` — Swift structs mirroring Python dataclasses
- `Services/Ghostty/` — Embedded terminal integration

## Wave Detail: Current + Runs

Wave detail now has two tabs:

- **Current** — active run state, output, commit/diff context, and run actions (`Land`, `Next`)
- **Runs** — historical run list with PR state, plus:
  - **Combine**: merge multiple open PRs into one

## Communication with lfd

Two patterns, intentionally different:

1. **HTTP services** (LocalWaveService.swift)
   - Reads waves + runs from lfd HTTP API
   - Used for primary wave data

2. **WebSocket subscription** (LocalEventService.swift)
   - Connects to `ws://127.0.0.1:2486/ws`
   - Subscribes to wave + agent + output events
   - Used for live UI updates

## UI Tests

```bash
./dev ui-test
```

Or via Xcode:
```bash
xcodegen generate
xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'
```
