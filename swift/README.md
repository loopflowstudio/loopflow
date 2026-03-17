# Concerto — Swift Apps

Visual interface for loopflow. SwiftUI today on macOS, with iOS support staged in.

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

Concerto bundles `lfd` and `lf` into the app bundle. By default, each opened repo starts a bundled `lfd` process with an ephemeral localhost port and token.

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

### Stream Logs

Long-running dev commands tee stdout to stable files under `~/.lf/logs/dev/`:

- `uv run python scripts/concerto-dev.py lfd` → `~/.lf/logs/dev/<repo>.lfd.log`
- `uv run python scripts/concerto-dev.py run-debug` → `~/.lf/logs/dev/<repo>.concerto-run-debug.log`

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

- `LoopflowCore/State/RepoState.swift` — shared app orchestrator for waves, connection, and stores
- `LoopflowCore/State/*.swift` — shared state containers (`WaveStore`, `RunStore`, `WorktreeStore`, `ConnectionStore`, `OutputBuffer`, `SessionState`)
- `LoopflowCore/Models` + `LoopflowCore/Services` — shared API models and transport/services
- `Concerto/Views` — mixed-platform views shared between iOS and macOS (`LiveOutput`, `WaveSessionView`)
- `Concerto/Platform/macOS` — macOS-only views, services, and keyboard handling
- `Concerto/Platform/iOS` — iOS-only views (`DiscoveryView`, `MobileWaveDetailView`, `MobileWaveListView`)
- `Concerto/Platform/macOS/Services/Ghostty` — embedded terminal integration (macOS-only)

## Multiplatform Boundary Rules

Use these rules for every new Swift change. The goal is low long-term `#if` footprint.

### 1) Put shared logic in LoopflowCore

- Shared models, state, and reusable views live in `LoopflowCore`.
- `LoopflowCore` must not import AppKit, Carbon, Ghostty, or other macOS-only frameworks.

### 2) Keep platform code in shell files

- Put macOS-only code under `Concerto/Platform/macOS/` (or existing macOS-specific folders such as `Services/Ghostty/`).
- Put iOS-only code under `Concerto/Platform/iOS/`.
- Prefer whole-file platform splits over inline branching.

### 3) Minimize `#if` usage

- Allowed: app entry wiring and platform shell files.
- Avoid: `#if` inside shared view/state/model files.
- If a feature needs platform behavior, inject a capability (protocol or environment action) from the shell.

### 4) Use capability injection, not platform checks in core

Examples:
- PR opening / external links
- local notifications behavior
- bundled-daemon availability

Define the capability in shared code, implement it in platform shell code.

### 5) Gate platform dependencies in Package.swift

- Platform-specific dependencies and linker settings must be conditionally applied.
- Shared targets should compile unchanged across supported platforms.

### Multiplatform PR checklist

- [ ] New shared code compiles without platform-only imports
- [ ] `#if` appears only in platform shell or app wiring files
- [ ] Platform behavior is injected through capability boundaries
- [ ] `Package.swift` platform gating is explicit and minimal

## Portfolio Dashboard

Concerto launches into a portfolio window instead of a single welcome panel:

- Each repo appears as a card with live wave status, blocked count, and diff totals
- Click a wave to open that repo window and focus the selected wave
- Use the `+` card to scan `~/src` and add another main git worktree
- Added repos persist between launches

## Attention Queue

Repo windows now open into a queue view when no wave is selected:

- Review-ready, failed, and queue-blocked waves are listed in urgency order
- Click an item to open its detail without drilling into the wave first
- Code review items offer `Ship`; step failures offer `Retry`
- Empty queues show `Nothing needs you. Waves are running.`

## Wave Detail: Current + Runs

Wave detail now has two tabs:

- **Current** — active run state, output, commit/diff context, and run actions (`Land`, `Next`)
- **Runs** — historical run list with PR state, plus:
  - **Combine**: merge multiple open PRs into one

## Session quote replies (macOS)

- Select text in an assistant bubble to open the reply popover
- Queue text replies, emoji reacts, and free-text notes in the draft tray
- Send once to assemble queued replies and composer text into one structured message

Open the prototype gallery from **Debug → Reply Demo** (`⇧⌘R`).

## Voice input (push-to-talk)

- Tap the mic button in the composer to start/stop recording
- Press and hold to record only while held
- Partial transcript appears under the composer while recording
- Final transcript is inserted into the composer for manual edit + send
- On macOS 26+/iOS 26+, Concerto uses Apple Dictation (`SpeechAnalyzer` + `DictationTranscriber`)
- On macOS 15–25/iOS 18–25, Concerto falls back to WhisperKit `tiny`
- Voice warmup runs at app launch to preinstall/prepare speech assets in the background
- If microphone permission is denied, Concerto shows an inline settings shortcut

## Keyboard Shortcuts

```text
J / K        Move wave focus down/up
Enter        Select focused wave
C            Create wave
E D R S L N  Edit/Delete/Retry/Stop/Land/Next
T I F P      Open Terminal/IDE/Finder/PR
1 / 2        Switch Current/Runs tab
/ or ⌘K      Open command palette
?            Show shortcut help
G H / G L    Jump to first/last wave
```

Shortcuts only run when a text field is not focused. Terminal input and command palette input keep their own key handling.

## Communication with lfd

Two patterns, intentionally different:

1. **HTTP services** (WaveService in `LocalWaveService.swift`)
   - Reads waves + runs from lfd HTTP API
   - Used for primary wave data

2. **WebSocket subscription** (EventService in `LocalEventService.swift`)
   - Connects to active server (`ws://.../ws` or `wss://.../ws`)
   - Uses the current connection credential (local session token or remote connection token)
   - Subscribes to wave + agent + output events
   - Used for live UI updates

Connection settings support two modes:

- **Bundled** (default): Concerto starts one bundled `lfd` process automatically.
- **Remote**: Concerto connects to an externally managed `lfd`.

On first launch (before any saved connection settings), Concerto also checks `~/.lf/concerto.yaml`:

```yaml
connection:
  host: lfd-dev.loopflow.studio
  port: 443
```

If present, it seeds remote mode from that host/port (TLS + static-token auth) and reads the token from Keychain via the existing `<host>:<port>` account mapping.

In bundled mode, Settings also supports optional CLI symlink install for `lf` + `lfd` (for `~/.local/bin` or `/usr/local/bin`).

## UI Tests

```bash
./dev ui-test
```

Or via Xcode:
```bash
xcodegen generate
xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```
