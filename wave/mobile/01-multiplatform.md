# 01: Multiplatform Concerto

Make Concerto build and run on iOS (iPhone + iPad) alongside macOS.

**Status: shipped** (branch `jack-heart.mobile.20260224_1845`)

## What shipped

Single-target multiplatform build with iOS shells and platform boundaries. The architecture diverged from the original plan in two meaningful ways:

**State stayed in Concerto.** RepoState and stores remain in `Concerto/State/`, not LoopflowCore. Platform-specific initializers (`RepoState+iOS.swift`, `RepoState+macOS.swift`) inject capabilities at the call site. This works — iOS shell doesn't need LoopflowCore to own the state to function as a remote client.

**iOS got its own views.** Instead of moving macOS views (WaveSidebar, WaveDetailPanel) into LoopflowCore and sharing them, iOS got purpose-built views: MobileWaveListView, MobileWaveDetailView, ConnectionSetupView. Mobile UX needs differ enough from desktop that sharing the same view hierarchy would have been forced.

**What moved to LoopflowCore:**
- Design tokens: BrandColors, DesignSystem (spacing, typography, colors, button styles)
- Models: ConnectionProfile, ServerConnection
- Services: unchanged (WaveService, AuthService, EventService already there)

**What shipped in Concerto:**
- `ConcertoApp.swift`: `#if os(macOS)/#else` at app entry — macOS multi-window, iOS single WindowGroup
- `Platform/iOS/`: MobileRootView (TabView phone / NavigationSplitView tablet), ConnectionSetupView, MobileWaveListView, MobileWaveDetailView, MobileConnectionProfilesStore, WaveStatus+Color, RepoState+iOS
- `Platform/macOS/`: LocalShellCommandRunner, RepoState+macOS
- Boundary enforcement: `scripts/check_swift_multiplatform_boundaries.py` blocks macOS-only imports in LoopflowCore and non-shell `#if` usage

**Validation:**
- `swift test --package-path swift` passes
- `check_swift_multiplatform_boundaries.py` passes
- Builds and runs on iPhone 17 and iPad Pro 11-inch (M5) simulators
- macOS behavior unchanged

## What remains

**State extraction** was the planned step 2 but was deferred. RepoState in LoopflowCore with public APIs would let Symphonia (separate repo) share state — that's the motivating use case, not iOS itself. Track as follow-up when Symphonia integration starts.

**Interactive end-to-end validation** is light. Build coverage is confirmed but full device flows (connection setup → connect to lfd → wave list → detail → output) haven't been validated against a live lfd on simulator.

## What to build

## Approach

### Package.swift changes

```swift
platforms: [.macOS(.v15), .iOS(.v18)]
```

Concerto target:
- Ghostty dependency: `condition: .when(platforms: [.macOS])`
- Carbon/Metal/IOKit linker settings: macOS-only condition
- `GHOSTTY_ENABLED` define: macOS-only condition

### LoopflowCore absorbs state + views

LoopflowCore becomes the full shared library. Everything that doesn't touch macOS-specific frameworks moves in:

**State (all made public):**
- RepoState (917 lines — the orchestrator, no macOS imports)
- WaveStore, RunStore, WorktreeStore
- OutputBuffer
- ChatState (+ ChatMessage, TranscriptEntry, ChatService protocol)
- ConnectionStore

**Views required in Stage 01:**
- WaveSidebar, WaveRow, WaveDetailPanel
- WaveChatView, ChatBubble, TranscriptItemCardView
- LiveOutput, FlowProgressPills, IterationTimeline
- NextActionsBar, WaitingStateCard
- StartWaveView, ConnectionSettingsView, DiagnosticsView
- DesignSystem, BrandColors, Typography

**Views explicitly deferred if not needed for iOS MVP flow:**
- Desktop-only helpers and secondary polish views that do not block:
  - iPhone: connection setup → wave list → wave detail/output
  - iPad: split layout without terminal/command palette/keyboard router

**What stays in Concerto (macOS-only):**
- EmbeddedTerminalPanel, TerminalTestWindow (Ghostty)
- KeyboardRouter, ShortcutHelpOverlay (NSEvent monitoring)
- CommandPalette (macOS keyboard-driven)
- ScreenshotWindow (NSWindow)
- WelcomeWindow (NSOpenPanel for repo picker)
- TerminalLauncher (NSWorkspace, Warp, Cursor)

Concerto app shell creates RepoState, injects LoopflowCore state types into the environment. Symphonia does the same with its own app shell.

### Platform-gate macOS code

```swift
#if os(macOS)
import Carbon
// keyboard router, ghostty, terminal launcher
#endif
```

ContentView needs platform variants:
- **macOS**: NavigationSplitView with sidebar (current behavior)
- **iPhone**: NavigationStack with wave list → push to detail
- **iPad**: NavigationSplitView similar to Mac but without keyboard router, terminal, command palette

Use `@Environment(\.horizontalSizeClass)` to distinguish phone from tablet within iOS.

### App entry point

ConcertoApp.swift needs platform branching:
- macOS: current WindowGroup setup with multiple windows
- iOS: single WindowGroup, tab-based navigation

```swift
@main
struct ConcertoApp: App {
    var body: some Scene {
        #if os(macOS)
        // existing multi-window setup
        #else
        WindowGroup {
            MobileRootView()
        }
        #endif
    }
}
```

### iPhone layout (MobileRootView)

Role model: ChatGPT iOS app.

```
TabView {
    WavesTab        // wave list → tap into wave detail
    SettingsTab     // connection settings, appearance
}
```

Wave detail on phone:
- Status header (wave name, status badge, branch)
- Live output (scrolling text, compact)
- Action buttons (prominent, bottom of screen — placeholder until stage 02)
- Chat button (opens sheet or pushes to chat view)
- No step runner, no typeaheads, no terminal

### iPad layout

Same NavigationSplitView as Mac but:
- No keyboard router
- No embedded terminal
- No command palette (use toolbar buttons instead)
- Touch-sized hit targets (44pt)

### Repo selection on iOS

macOS uses NSOpenPanel. iOS options:
- Recent repos list (like WelcomeWindow) + manual URL entry
- Connect to a running lfd by entering host:port
- No local file picker (iOS doesn't have local git repos)

This means iOS is always a remote client connecting to lfd. The "repo" concept becomes "lfd connection."
Running `lfd` as a standalone local daemon on iPhone/iPad is out of scope.

## Implementation order (ship in seams)

1. Package/platform gating compiles for iOS.
2. State extraction + capability boundaries (`RepoState`/stores in LoopflowCore).
3. Shared views required for iOS MVP flow.
4. iOS shell wiring (phone + tablet navigation).
5. Resource parity + regression pass.

Each seam should be independently buildable before moving to the next.

## Boundary enforcement

- Put platform-specific code in platform shell files, not shared files.
- Keep long-term `#if` footprint low:
  - allowed in app entry wiring and platform shell files
  - avoid in shared LoopflowCore state/views/models
- Inject platform behavior (daemon, notifications, external actions) through capability boundaries instead of inline platform checks.

### New types

```swift
// In LoopflowCore — saved connection for iOS "connect to lfd" flow
public struct ConnectionProfile: Codable, Identifiable, Sendable, Hashable {
    public let id: UUID
    public var name: String           // "My Mac", "Dev Server"
    public var connection: ServerConnection
    public var lastConnectedAt: Date?
}
```

## Constraints

- Mac behavior must not change — all existing Mac UX preserved
- LoopflowCore must not depend on any macOS-specific frameworks
- Font resources (Cormorant Garamond, Lato, JetBrains Mono) must be available to LoopflowCore
- No discovery flow in Stage 01 (handled in 04-lfd-discovery)

## Done when

```bash
# builds for iOS simulator
swift build --sdk ... # or via Xcode
# runs on iPhone simulator — shows connection screen, can connect to lfd, see wave list
# runs on iPad simulator — shows sidebar + detail
# Mac behavior unchanged — all existing tests pass
swift test --package-path swift
```

Boundary checks also pass:
- No macOS-only imports in `LoopflowCore`
- No net-new non-shell `#if` usage

## Post-ship notes

**Achieved:** builds, simulators, boundary checks, macOS regression pass all green.

**Deferred:** state extraction to LoopflowCore. Not needed for iOS remote client — motivating use case is Symphonia sharing state, which is future work. Revisit when Symphonia integration begins.

**Learned:** purpose-built mobile views are simpler than sharing desktop views through LoopflowCore. This means Stage 02 action buttons need to land in both MobileWaveDetailView (iOS) and WaveChatView (macOS), but the shared ActionButtonsView component can still live in LoopflowCore.
