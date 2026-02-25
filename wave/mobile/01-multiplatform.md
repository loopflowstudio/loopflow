# 01: Multiplatform Concerto

Make Concerto build and run on iOS (iPhone + iPad) alongside macOS.

## What to build

Concerto becomes a multiplatform SwiftUI app. One target, one scheme, three form factors. LoopflowCore absorbs everything shared — models, services, state (including RepoState), and reusable views. Concerto becomes a thin app shell with platform-specific code. macOS-specific code (Ghostty, keyboard router, Carbon frameworks) gets platform-gated.

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

**Views:**
- WaveSidebar, WaveRow, WaveDetailPanel
- WaveChatView, ChatBubble, TranscriptItemCardView
- LiveOutput, FlowProgressPills, IterationTimeline
- NextActionsBar, WaitingStateCard
- StartWaveView, ConnectionSettingsView, DiagnosticsView
- DesignSystem, BrandColors, Typography

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

## Done when

```bash
# builds for iOS simulator
swift build --sdk ... # or via Xcode
# runs on iPhone simulator — shows connection screen, can connect to lfd, see wave list
# runs on iPad simulator — shows sidebar + detail
# Mac behavior unchanged — all existing tests pass
swift test --package-path swift
```
