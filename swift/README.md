# Loopflow — Swift Apps

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

The `./dev` script is the primary build tool. It uses Swift Package Manager (`Package.swift`) for building and installs to `~/Applications/Loopflow Dev.app` to preserve macOS permissions across rebuilds.

Loopflow bundles `lfd` and `lf` into the app bundle. By default, each opened repo starts a bundled `lfd` process with an ephemeral localhost port and token.
Bundled macOS launches mark their child daemon/container processes so Loopflow can reap stale instances on the next start and shut the bundled `lfd` down when the app exits.

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

- `uv run python scripts/loopflow-dev.py lfd` → `~/.lf/logs/dev/<repo>.lfd.log`
- `uv run python scripts/loopflow-dev.py run-debug` → `~/.lf/logs/dev/<repo>.loopflow-run-debug.log`

### Xcode Project

The `project.yml` (XcodeGen) generates `LoopflowSwift.xcodeproj` for Xcode development and UI tests. Both build systems (SPM and XcodeGen) should stay in sync.

```bash
# Regenerate Xcode project
xcodegen generate

# Build with Xcode
xcodebuild -scheme LoopflowMac -destination 'platform=macOS' build
```

## Embedded Terminal (Ghostty)

Loopflow embeds Ghostty for terminal functionality.

- Ordinary typing stays on Ghostty's key-event path, so terminal apps see keystrokes instead of paste-like text injection
- IME commit and explicit paste still use the text-input path
- Loopflow keeps only intentional pane-management shortcuts (`⌃⇧5`, `⌃⇧'`, `⇧⌘↩`, `⌘W`, `⌥⌘←/→`) above the terminal surface

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

See `LoopflowMac/Services/Ghostty/README.md` for integration details.

## Architecture

- `Loopflow/State/RepoState.swift` — shared app orchestrator for waves, connection, and stores
- `Loopflow/State/*.swift` — shared state containers (`WaveStore`, `RunStore`, `WorktreeStore`, `ConnectionStore`, `OutputBuffer`, `SessionState`)
- `Loopflow/Models` + `Loopflow/Services` — shared API models and transport/services
- `Loopflow/Views` — mixed-platform views shared between iOS and macOS (`LiveOutput`, `WaveSessionView`)
- `LoopflowMac` — macOS-only views, services, and keyboard handling
- `LoopflowiOS` — iOS-only views (`DiscoveryView`, `MobileWaveDetailView`, `MobileWaveListView`)
- `LoopflowMac/Services/Ghostty` — embedded terminal integration (macOS-only)

## Multiplatform Boundary Rules

Use these rules for every new Swift change. The goal is low long-term `#if` footprint.

### 1) Put shared logic in Loopflow

- Shared models, state, and reusable views live in `Loopflow`.
- `Loopflow` must not import AppKit, Carbon, Ghostty, or other macOS-only frameworks.

### 2) Keep platform code in shell files

- Put macOS-only code under `LoopflowMac/` (or existing macOS-specific folders such as `Services/Ghostty/`).
- Put iOS-only code under `LoopflowiOS/`.
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

Loopflow launches into a portfolio window instead of a single welcome panel:

- Each repo appears as a card with live wave status, blocked count, and diff totals
- Click a repo to open its repo-scoped wave list
- Use the `+` card to scan `~/src` and add another main git worktree
- Added repos persist between launches

## Repo Wave List

Repo windows open into the first slice of the outward wave model:

- Shows waves touching the current repo
- Renders each row with wave name, repo chip, and rollup status
- Keeps create-wave, open-wave detail, and in-repo workspace flows out of the exposed path for now

## Wave Workspace

The multiplexer workspace remains available in the codebase, but repo windows no longer expose it as the default flow:

- Default panes show a minimal **Roadmap** list, **Roadmap Detail**, and a Ghostty-backed **Terminal**
- Roadmap rows keep just the title and priority, sort shipped items to the bottom, and reveal an inline play button on hover
- The selected roadmap item renders its full markdown in **Roadmap Detail**, with an always-visible **Build** action
- `j`/`k`, `↑`/`↓`, and `Return` work directly in the roadmap list for keyboard-first triage
- Wave taglines now come from the opening paragraph of `GOAL.md` when present
- Local worktrees expose **Open Terminal** and **Open Internally** actions that both attach the same tmux-backed shell
- Cmd+K switches waves and opens or focuses panes like **README**, **Runs**, and **Launcher**
- Waves without worktrees still keep roadmap/readme/detail panes available; only terminal-style panes show a worktree placeholder
- Interactive sessions still take over the workspace when a flow needs input

## Flow Catalog

Repo windows also have a **Flows** tab:

- Left pane groups flows and skills into **Build**, **Govern**, and **Ops**
- Expand a flow inline to see nested flows, xor branches, and loops
- Click any flow or skill to see every parent flow that uses it
- Repo `.lf/flows/*.yaml` and `.lf/skills/*.md` overrides replace builtins in place and get repo-source styling
- `Loopflow/Models/Catalog.swift` mirrors the flow catalog DTO; registry reads go through `lf --json`

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
- On macOS 26+/iOS 26+, Loopflow uses Apple Dictation (`SpeechAnalyzer` + `DictationTranscriber`)
- On macOS 15–25/iOS 18–25, Loopflow falls back to WhisperKit `tiny`
- Voice warmup runs at app launch to preinstall/prepare speech assets in the background
- If microphone permission is denied, Loopflow shows an inline settings shortcut

## Keyboard Shortcuts

```text
J / K        Move wave focus down/up
Enter        Select focused wave
C            Create wave
D R S L N    Delete/Retry/Stop/Land/Next
T I F P      Open Terminal/IDE/Finder/PR
1 / 2        Switch Current/Runs tab
/ or ⌘K      Open command palette
?            Show shortcut help
G H / G L    Jump to first/last wave
```

Shortcuts only run when a text field is not focused. Terminal input and command palette input keep their own key handling.

## Communication with lfd

Two patterns, intentionally different — split by durability (see
`scratch/eventing.md`): durable facts are QUERIES, live motion is a per-wave
stream. There is no machine-wide telemetry socket.

1. **Registry queries** (`RegistryQuery`)
   - Discovery + history: which waves exist (running and stopped), a wave's
     runs, its attention, and telemetry traces — `lf ls/status/runs/trace/doctor
     --json` over `lfdb`
   - A point-in-time snapshot, re-run on a cadence; not a stream

Open **Go → Telemetry** for the selected run's process flamechart, additive
cost waterfall, cache-hit history, and seven-day silence ribbon.

2. **Per-wave SSE** (`WaveChatConnection` in `WaveChatClient.swift`)
   - One connection per wave the UI is watching, off that wave's `/events`
   - Frames: `state` / `turn` / `memory` / `op` (run/flow/skill motion)
   - Drives both the chat pane and the wave's dashboard card

## Connections Panel

Providers are grouped by role (Agents, Source Control, Project Management, Secrets). Each group renders as a `ProviderGroupSection` containing `ProviderRow` items with status dot, auth action, and optional enable/disable toggle.

The Secrets group (Doppler) expands inline when connected to show project/config selection, key status, and sync controls. Smart defaults pick `dev > prd > prod` when loading configs.

`ConnectionsPanel` is shared between repo-level settings (`ConnectionSettingsView`) and the portfolio toolbar sheet (`PortfolioConnectionsSheet`).

## Communication with lfd

Connection settings support two modes:

- **Bundled** (default): Loopflow starts one bundled `lfd` process automatically.
- **Remote**: Loopflow connects to an externally managed `lfd`.

On first launch (before any saved connection settings), Loopflow also checks `~/.lf/loopflow.yaml`:

```yaml
connection:
  host: lfd.example.com
  port: 443
  token: "paste-token-here"
```

If present, it seeds remote mode from that host/port (TLS + static-token auth). When `token` is set, Loopflow reads it fresh from the file for matching host/port requests so token rotation does not require re-pasting through Settings. Without `token`, Loopflow falls back to Keychain via the existing `<host>:<port>` account mapping.

In bundled mode, Settings also supports optional CLI symlink install for `lf` + `lfd` (for `~/.local/bin` or `/usr/local/bin`).

## UI Tests

```bash
./dev ui-test
```

Or via Xcode:
```bash
xcodegen generate
xcodebuild test -project LoopflowSwift.xcodeproj -scheme LoopflowMac -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```
