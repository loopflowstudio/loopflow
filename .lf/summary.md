# swift/ — Concerto, Symphonia, LoopflowCore

macOS 15+ SwiftUI apps for managing AI coding waves. Three SPM targets sharing a core library.

## Package Structure

```
swift/Package.swift          — Swift 6.0, macOS 15+
  Products: LoopflowCore (lib), Concerto (exe), Symphonia (exe)
  Dependencies: ViewInspector 0.10+, GhosttyKit (binary xcframework)
  Concerto links: Carbon, QuartzCore, Metal, IOKit, c++
  Concerto defines: GHOSTTY_ENABLED
```

```
swift/
  LoopflowCore/             — Shared models + services
    Models/                  — Domain types
    Services/                — Auth, events, wave API, logging, notifications
  Concerto/                  — Main macOS app
    Models/                  — App-specific models
    Services/                — Terminal, snapshots, setup, Ghostty
    Services/Ghostty/        — Embedded terminal via GhosttyKit
    State/                   — Observable stores (WaveStore, RunStore, OutputBuffer, RepoState)
    Views/                   — All SwiftUI views
  Symphonia/                 — Alternate/placeholder app
  ConcertoTests/             — Unit tests (ViewInspector)
  ConcertoUITests/           — Screenshot pipeline tests
  SymphoniaTests/            — Placeholder tests
```

## Constants

```swift
// LoopflowCore/LoopflowCore.swift
let lfdDefaultPort: Int = 2486
let lfdBaseURL: URL = "http://127.0.0.1:2486"
let lfdApiBaseURL: URL = "http://127.0.0.1:2486/v0"
```

## Core Data Structures

### Wave (the central domain object)

```swift
// LoopflowCore/Models/Wave.swift
struct Wave: Sendable, Identifiable, Hashable {
    let id: String
    var name: String
    var repo: String
    var flow: String
    var direction: [String]
    var area: [String]
    var stimuli: [Stimulus]
    var status: WaveStatus           // idle | running | waiting | failed | paused
    var iteration: Int
    var localWorktree: String?
    var remoteBranch: String?
    var commits: [CommitEntry]
    var diffStat: String?
    var flowSteps: [String]
    var openPRCount: Int
    var activeRun: WaveRun?
    var createdAt: Date?
}

enum WaveStatus: String, Sendable, Codable {
    case idle, running, waiting, failed, paused
    var color: Color      // green/blue/orange/red/gray
    var icon: String      // SF Symbols
}

struct Stimulus: Sendable, Hashable, Codable, Identifiable {
    let id: String
    let kind: Kind        // loop | watch | cron
    let cron: String?
    enum Kind: String, Sendable, Codable, CaseIterable {
        case loop, watch, cron
        var icon: String; var label: String
    }
}

enum MergeMode: String, Sendable, Codable { case pr, land }

enum WaitingReason: Sendable, Hashable {
    case prLimitReached(open: Int, limit: Int)
    var description: String; var accessibilityDescription: String
}

struct CommitEntry: Sendable, Hashable, Identifiable {
    let sha: String; let message: String
}

struct InteractiveSession: Sendable, Identifiable {
    let id: String; let waveId: String; let step: String
    let worktreePath: String; let prompt: String?; let startedAt: Date
    var command: String   // builds "lf <step>" shell command
}
```

### WaveRun

```swift
// LoopflowCore/Models/WaveRun.swift
struct WaveRun: Sendable, Identifiable, Hashable {
    let id: String
    var waveId: String?
    var flow: String; var area: String; var repo: String
    var direction: [String]
    var status: WaveRunStatus        // pending | running | waiting | completed | failed | cancelled
    var iteration: Int; var stepIndex: Int
    var worktree: String?; var branch: String?
    var currentStep: String?; var error: String?
    var pr: PullRequest?
    var startedAt: Date?; var endedAt: Date?; var createdAt: Date
    var duration: String?            // "Xm##s"
    var relativeTime: String         // relative date
}

enum WaveRunStatus: String, Sendable, Codable {
    case pending, running, waiting, completed, failed, cancelled
    var color: Color
}
```

### WaveViewModel (enriched wave for UI)

```swift
// LoopflowCore/Models/WaveViewModel.swift
struct WaveViewModel: Sendable, Identifiable, Hashable {
    var api: Wave                    // underlying API model
    var worktreePath: String?; var branch: String?
    var isDirty: Bool; var isRebasing: Bool; var isMerging: Bool; var hasDiff: Bool
    var aheadMain: Int; var behindMain: Int; var aheadRemote: Int; var behindRemote: Int
    var prURL: URL?; var prNumber: Int?; var prState: PRState?
    var recentSteps: [StepRun]
    var prLimit: Int; var mergeMode: MergeMode; var pid: Int?
    var lastMainSha: String?; var waitingReason: WaitingReason?; var runStartedAt: Date?
    // Many computed: id, name, repo, flow, direction, area, stimuli, status, iteration,
    // activeRun, createdAt, stepIndex, shortId, displayName, areaDisplay, directionDisplay,
    // commits, diffStat, flowSteps, openPRCount, effectiveOpenPRCount, hasOpenPRs,
    // statusText, iterationText, detailText, stimulusText, hasActiveStimulus,
    // statusIndicator: (icon: String, color: Color),
    // pendingPR: (number: Int, url: URL?)?,
    // lastActivityAt: Date?, lastActivityDescription: String?
}
```

### Flow + Step

```swift
// LoopflowCore/Models/Flow.swift
enum FlowType: String, Sendable, Codable { case flow, step }

struct Flow: Sendable, Codable, Identifiable, Equatable {
    let id: UUID; var name: String; var steps: [Step]; var type: FlowType
}

// LoopflowCore/Models/Step.swift
struct Step: Sendable, Codable, Equatable, Identifiable {
    let id: UUID; var prompt: String; var config: StepConfig?
}
struct StepConfig: Sendable, Codable, Equatable {
    var model: String?; var direction: String?; var context: [String]?
    var isEmpty: Bool
}
struct StepRun: Sendable, Identifiable, Codable, Hashable {
    let id: String; var step: String; var repo: String; var worktree: String
    var status: String; var startedAt: Date; var endedAt: Date?
    var model: String; var runMode: String
    var isRunning: Bool; var isCompleted: Bool; var isError: Bool
}
```

### PullRequest

```swift
// LoopflowCore/Models/PullRequest.swift
enum PRState: String, Sendable, Codable { case open, merged, closed, draft }
struct PullRequest: Sendable, Hashable, Codable {
    var url: URL; var number: Int?; var state: PRState?
    var title: String?; var branch: String?
}
```

### Preferences

```swift
// LoopflowCore/Models/AppPreferences.swift
enum TerminalApp: String, Sendable, CaseIterable { case warp, iterm, terminal, kitty }
enum IDEApp: String, Sendable, CaseIterable { case cursor, vscode, zed }

// LoopflowCore/Models/AppearanceMode.swift
enum AppearanceMode: String, Sendable, CaseIterable {
    case system, light, dark
    var colorScheme: ColorScheme?  // nil for system
}
```

## Services (LoopflowCore)

### LocalWaveService — HTTP REST client to lfd

```swift
// LoopflowCore/Services/LocalWaveService.swift
struct LocalWaveService: WaveServiceProtocol, @unchecked Sendable {
    func listWaves(repo: URL) async throws -> [Wave]
    func getWave(_ id: String) async throws -> Wave
    func listFlowsAndDirections(repo: URL) async throws -> WaveFlowsResult
    func listWaveRuns(waveId: String?, repo: URL?, limit: Int) async throws -> [WaveRun]
    func connectLfd() async throws
    func checkAvailability() async -> Bool
    func createWave(name: String, repo: URL) async throws -> Wave
    func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave
    func run(_ id: String, overrides: RunOverrides?) async throws
    func addStimulus(_ waveId: String, kind: Stimulus.Kind, cron: String?) async throws -> Stimulus
    func removeStimulus(_ waveId: String, stimulusId: String) async throws
    func connect(_ id: String) async throws -> ConnectionInfo
    func stop(_ id: String) async throws
    func landWave(_ id: String) async throws
    func nextWave(_ id: String) async throws -> String
    func cloneWave(_ id: String, name: String?) async throws -> Wave
    func deleteWave(_ id: String) async throws
    func collapsePRs(_ id: String) async throws -> CollapsePRsResult
    func absorbIntoPR(_ id: String, prNumber: Int) async throws -> AbsorbIntoPRResult
    func streamOutput(waveId: String) -> AsyncThrowingStream<String, Error>
}

struct WaveConfigUpdate: Sendable {
    var name: String?; var area: [String]?; var direction: [String]?
    var flow: String?; var status: WaveStatus?
}
struct RunOverrides: Sendable {
    var area: [String]?; var direction: [String]?; var flow: String?
}
struct ConnectionInfo: Sendable {
    var worktree: String; var step: String; var agentId: String
    var promptFile: String; var waveRunId: String?; var stepIndex: Int
}
struct CollapsePRsResult: Sendable { var newPRUrl: String?; var closedPRs: [Int] }
struct AbsorbIntoPRResult: Sendable { var targetBranch: String; var commitsAbsorbed: Int }
```

### LocalEventService — WebSocket event stream

```swift
// LoopflowCore/Services/LocalEventService.swift
actor LocalEventService {
    var isConnected: Bool
    func subscribe(onEvent: @Sendable (LFDEvent) -> Void,
                   onConnectionChange: @Sendable (Bool) -> Void) async
    func disconnect() async
}

enum LFDEvent: Sendable {
    case connected(ConnectedEvent)      // initial wave list
    case wave(WaveEvent)                // created | updated | deleted | started | stopped | waiting
    case worktree(WorktreeEvent)
    case agentStarted(AgentStartedEvent)
    case agentEnded(AgentEndedEvent)
    case output(OutputEvent)            // streaming agent output
}
struct ConnectedEvent: Sendable { let timestamp: Date; let waves: [Wave] }
struct WaveEvent: Sendable {
    let type: WaveEventType; let waveId: String; let waveRunId: String?
    let step: String?; let name: String?; let wave: Wave?; let timestamp: Date
}
enum WaveEventType: String, Sendable {
    case created, updated, deleted, started, stopped, waiting
}
struct WorktreeEvent: Sendable { let worktree: String; let repo: String; let branch: String?; let timestamp: Date }
struct AgentStartedEvent: Sendable { let agentId: String; let step: String; let worktree: String; let timestamp: Date }
struct AgentEndedEvent: Sendable { let agentId: String; let status: String; let timestamp: Date }
struct OutputEvent: Sendable { let waveId: String; let agentId: String; let text: String; let timestamp: Date }
```

### Auth

```swift
// LoopflowCore/Services/AuthService.swift
class AuthService: NSObject, @unchecked Sendable {
    func signIn() async throws -> String        // OAuth via ASWebAuthenticationSession
    func signOut() throws                        // Clear keychain
    func currentToken() -> String?
    func refreshToken() async throws -> String
}

// LoopflowCore/Services/AuthState.swift — @MainActor @Observable
class AuthState {
    var token: String?; var isLoading: Bool; var error: AuthError?
    var isAuthenticated: Bool; var isExpired: Bool; var needsRefresh: Bool
    func signIn() async; func signOut()
}

// LoopflowCore/Services/AuthError.swift
enum AuthError: Error, Sendable, LocalizedError {
    case noCallback, invalidCallback, notAuthenticated, tokenExpired
    case sessionFailed, refreshFailed(String)
    case keychainWrite(OSStatus), keychainDelete(OSStatus), unknown(Error)
}
```

### Other Services

```swift
// LoopflowCore/Services/NotificationService.swift
class NotificationService: NSObject, UNUserNotificationCenterDelegate, @unchecked Sendable {
    static let shared: NotificationService
    func requestAuthorization() async throws
    func notifyNeedsInteractive(waveId: String, waveName: String, step: String)
    func notifyError(waveId: String, waveName: String, message: String)
    func notifyPRReady(waveId: String, waveName: String, prNumber: Int)
}

// LoopflowCore/Services/LoggingService.swift
enum LoggingService {
    enum Category: String { case worktrees, lfd, general, ui, model }
    static func append(_ message: String, category: Category = .worktrees)
    static func ui(_ message: String); static func model(_ message: String); static func lfd(_ message: String)
    static func read(category: Category) -> String
    static func logDirectory() -> URL   // ~/Library/Logs/Concerto/
}

// LoopflowCore/Services/TokenProvider.swift
protocol TokenProvider: Sendable { func token() async throws -> String }
struct NoAuthProvider: TokenProvider { }
class KeychainTokenProvider: TokenProvider, @unchecked Sendable { }

// LoopflowCore/Services/WaveServiceProtocol.swift
struct WaveFlowsResult: Sendable { var flows: [Flow]; var directions: [String] }
protocol WaveServiceProtocol: Sendable { /* same methods as LocalWaveService */ }
```

## State Layer (Concerto)

### RepoState — central app state coordinator

```swift
// Concerto/State/RepoState.swift
@MainActor @Observable
final class RepoState {
    var currentRepo: URL?
    var flows: [Flow]; var availableDirections: [String]
    let waveStore: WaveStore; let runStore: RunStore
    var waves: [WaveViewModel]; var waveGroups: WaveGroups
    var selectedWaveId: String?; var selectedWave: WaveViewModel?
    var isLoading: Bool; var errorMessage: String?; var lfdConnected: Bool

    // Repo lifecycle
    func openRepo(_ url: URL, outputBuffer: OutputBuffer, skipBackgroundRefresh: Bool = false) async
    func startEventSubscription(outputBuffer: OutputBuffer)

    // Wave CRUD — all use optimistic updates with rollback
    func createWave(name: String) async throws
    func runWave(wave: WaveViewModel, area: [String]?, direction: [String]?, flow: String?) async throws
    func stopWave(_ wave: WaveViewModel) async throws
    func deleteWave(_ wave: WaveViewModel) async throws
    func renameWave(_ wave: WaveViewModel, to newName: String) async throws
    func updateWave(_ wave: WaveViewModel, area: [String]?, direction: [String]?, flow: String?, status: WaveStatus?) async throws
    func landWave(_ wave: WaveViewModel) async throws
    func nextWave(_ wave: WaveViewModel) async throws
    func cloneWave(_ wave: WaveViewModel) async throws -> WaveViewModel
    func addStimulus(wave: WaveViewModel, kind: Stimulus.Kind, cron: String?) async throws
    func removeStimulus(wave: WaveViewModel, stimulusId: String) async throws
    func collapsePRs(_ waveId: String) async throws -> CollapsePRsResult
    func absorbIntoPR(_ waveId: String, prNumber: Int) async throws -> AbsorbIntoPRResult
    func connectLfd(outputBuffer: OutputBuffer) async throws

    // Screenshot/test modes
    enum UITestMode: String { case emptyWorkspaces, sampleWorkspaces, mockWaves }
    struct ScreenshotMode { let outputPath: String; let repoPath: String?; ... ; static func fromArgs() -> ScreenshotMode? }
    func configureMockWaves(); func configureMockWavesEmpty()
}
```

### WaveStore — observable wave collection

```swift
// Concerto/State/WaveStore.swift
struct WaveGroups {
    var blocked: [WaveViewModel]; var pr: [WaveViewModel]
    var recentActivity: [WaveViewModel]; var active: [WaveViewModel]; var idle: [WaveViewModel]
    var attentionCount: Int; var openPRCount: Int; var allInOrder: [WaveViewModel]
}

@MainActor @Observable
class WaveStore {
    var waves: [String: WaveViewModel]; var ordered: [WaveViewModel]; var groups: WaveGroups
    var onStatusChange: ((WaveViewModel, WaveStatus?, WaveStatus) -> Void)?
    func set(_ wave: WaveViewModel); func setAll(_ newWaves: [WaveViewModel])
    func remove(_ id: String) -> WaveViewModel?; func removeAll()
    // Optimistic mutation support
    func applyOptimistic(_ id: String, _ mutation: (inout WaveViewModel) -> Void) -> WaveViewModel?
    func commitMutation(_ id: String); func rollback(_ snapshot: WaveViewModel)
    // Pending wave support (pre-server-creation)
    func insertPending(_ wave: WaveViewModel); func replacePending(_ pendingId: String, with wave: WaveViewModel)
    func removePending(_ id: String); func applyDelete(_ id: String)
}
```

### RunStore + OutputBuffer

```swift
// Concerto/State/RunStore.swift
@MainActor @Observable class RunStore {
    var runs: [String: [WaveRun]]
    func setRuns(for waveId: String, _ newRuns: [WaveRun])
    func runs(for waveId: String) -> [WaveRun]
    func clear(for waveId: String)
}

// Concerto/State/OutputBuffer.swift
struct OutputLine: Identifiable { let id: UUID; let text: String; let timestamp: Date }
@MainActor @Observable class OutputBuffer {
    var interactiveSession: InteractiveSession?
    func appendOutput(waveId: String, text: String, timestamp: Date)
    func output(for waveId: String) -> [OutputLine]
    func clearOutput(for waveId: String)
    func startStreaming(waveId: String); func stopStreaming(waveId: String)
    func recentOutput(for waveId: String, maxLength: Int = 60) -> String?
    func launchInteractiveSession(waveId: String, step: String, worktreePath: String, prompt: String?)
    func endInteractiveSession(); func hasActiveSession(for waveId: String) -> Bool
}
```

## Concerto App + Services

### App Entry

```swift
// Concerto/ConcertoApp.swift — @main
struct ConcertoApp: App {
    // Scenes: default WindowGroup (Welcome/Repo/Screenshot), "repo" WindowGroup, "terminal-test" Window
    // Commands: beta toggle, appearance picker, Snapshot (⌘4), Command Palette (⌘K), Terminal Test (⌘⇧T)
}
```

### Design System

```swift
// Concerto/DesignSystem.swift
enum Spacing     { static let xxs=2, xs=4, sm=8, md=12, lg=16, xl=20, xxl=24, xxxl=32 }
enum HitTarget   { static let minimum=24, comfortable=32, touch=44 }
enum CornerRadius{ static let sm=4, md=8, lg=12, xl=16, full=9999 }
enum Typography  { static func heroTitle/sectionTitle/body/bodyBold/caption/code/codeSmall -> Font }
enum DesignAnimation { static func standard/fast/spring(reduceMotion:) -> Animation }

// Concerto/BrandColors.swift
// Color.loopflowBurgundy(0x722F37), .loopflowCream(0xFAF8F5), .loopflowSlate(0x2B3036), etc.
struct LoopflowPalette {
    let background, surface, surfaceMuted, border, text, textSecondary, accent, accentHover: Color
    static func make(for scheme: ColorScheme) -> LoopflowPalette
}
```

### Ghostty (embedded terminal)

```swift
// Concerto/Services/Ghostty/GhosttyManager.swift — @MainActor, ObservableObject
class GhosttyManager {
    enum State: Equatable { case uninitialized, initializing, ready, failed(String) }
    static let shared: GhosttyManager
    func initialize(); func tick()
    func createSurface(workingDirectory: String, command: String?, view: NSView) -> ghostty_surface_t?
    func destroySurface(_ surface: ghostty_surface_t)
    func registerActiveSession(_ surface: ghostty_surface_t, sessionId: String)
    func destroyActiveSession(); func sendText(_ text: String)
}

// Concerto/Services/Ghostty/GhosttyTerminalView.swift
struct GhosttyTerminalView: View { /* NSViewRepresentable wrapping GhosttyMetalView */ }
class GhosttyMetalView: NSView, NSTextInputClient { /* keyboard, mouse, IME, surface lifecycle */ }

// Concerto/Services/Ghostty/GhosttyTypes.swift
enum TerminalStatus: Equatable {
    case initializing, running, completed(exitCode: Int32), failed(error: String)
}
struct GhosttySession: Identifiable {
    let id: String; let worktree: String; let command: [String]
    var status: TerminalStatus; var surface: ghostty_surface_t?
}
```

### Other Concerto Services

```swift
// Concerto/Services/TerminalLauncher.swift
struct TerminalLauncher {
    func launchTerminal(_ terminal: TerminalApp, at path: URL, command: String?) throws
    func openInIDE(_ ide: IDEApp, at path: URL, workspace: String?) throws
    func openInFinder(at path: URL); func openURL(_ url: URL)
    func launchStep(_ step: String, terminal: TerminalApp, at repo: URL) throws
}

// Concerto/Services/SnapshotService.swift — @MainActor
struct SnapshotService {
    func snapshotKeyWindow() throws -> URL
    func snapshotKeyWindow(to outputPath: String) throws -> URL
    func snapshotWindow(_ window: NSWindow, to outputPath: String) throws -> URL
}

// Concerto/Services/SetupService.swift
struct SetupService {
    struct DependencyStatus { var lfInstalled: Bool; var lfPath: String? }
    func checkDependencies() -> DependencyStatus
    func install() async throws; func ensureDaemonRunning() async throws
}

// Concerto/Services/RecentsService.swift — @Observable
class RecentsService { var recentRepos: [RecentRepo]; func addRecent/removeRecent/clearAll }

// Concerto/Services/NameGenerator.swift
enum NameGenerator { static func generate() -> String }  // "magical-musical" pairs

// Concerto/Services/AppIconProvider.swift
enum AppIdentifier { case cursor, warp, vscode, iterm, terminal, zed, kitty, github }
struct AppIconProvider { static func icon(for: AppIdentifier) -> NSImage? }

// Concerto/Models/RecentRepo.swift
struct RecentRepo: Codable, Identifiable { let path: String; let lastOpened: Date }
```

## Views (Concerto)

| View | Purpose |
|------|---------|
| `WelcomeWindow` | Repo picker with recents list |
| `RepoWindow` | Per-repo window wrapper, injects `RepoState` + `OutputBuffer` |
| `ContentView` | Main layout: sidebar + detail pane |
| `WaveSidebar` | Wave list grouped by `WaveGroups` sections |
| `WaveRow` | Single wave in sidebar: status indicator, name, inline rename, live output preview |
| `WaveDetailPanel` | Detail pane with tabs: current config vs. run history |
| `StepRunner` | Area/direction/flow config, run/auto buttons, stimulus management |
| `NextActionsBar` | Context-aware action buttons for selected wave |
| `FlowProgressPills` | Step progress indicator with elapsed time |
| `LiveOutput` | Scrolling agent output lines |
| `WaveRunsTab` | Run history table, PR collapse/absorb actions |
| `IterationTimeline` | Dot timeline of wave iterations |
| `WaitingStateCard` | Displayed when wave is waiting (PR limit reached) |
| `InteractiveSessionView` | Embedded terminal for interactive agent steps |
| `EmbeddedTerminalPanel` | Ghostty terminal panel in detail view |
| `CommandPalette` | ⌘K fuzzy search over actions |
| `QuickExperimentView` | Quick-launch predefined steps |
| `AreaTypeahead` / `DirectionTypeahead` / `FlowTypeahead` | Typeahead selectors for wave config |
| `SetupView` | First-run dependency check + install |
| `DiagnosticsView` | Log viewer |
| `ScreenshotWindow` | Automated screenshot capture for docs |
| `TerminalTestWindow` | Ghostty terminal testing |

## Key Patterns

1. **Optimistic updates**: `WaveStore.applyOptimistic` snapshots state, mutates locally, calls API, rolls back on failure
2. **WebSocket events**: `LocalEventService` streams `LFDEvent`s; `RepoState.startEventSubscription` applies them to stores
3. **Wave grouping**: `WaveGroups` sorts waves into blocked/pr/recentActivity/active/idle for sidebar sections
4. **Conditional compilation**: `#if GHOSTTY_ENABLED` gates all Ghostty code; stubs provided when disabled
5. **@Observable**: State classes use Swift 5.9 `@Observable` macro (not Combine `@Published`)
6. **Actor isolation**: `LocalEventService` is an actor; `RepoState`, stores, views are `@MainActor`
7. **Pending waves**: `WaveStore.insertPending` creates local-only waves before server confirms creation

## Testing

```bash
swift test --package-path swift
```

Tests: `AuthServiceTests`, `GhosttyTests`, `RunStoreTests`, `WaveRowTests`, `WaveStoreTests`, `WaveTests`, `ScreenshotPipelineTests`
