# Swift Codebase Summary

## Package Structure

```
swift/Package.swift
  Package: LoopflowSwift
  Platforms: macOS 15+
  Products:
    - LoopflowCore (library)   — shared models and services
    - Concerto (executable)    — main macOS app
    - Symphonia (executable)   — secondary app (minimal)
  Dependencies:
    - ViewInspector 0.10.0     — SwiftUI view testing
  Binary targets:
    - GhosttyKit xcframework   — embedded terminal
  Concerto links: Carbon, QuartzCore, Metal, IOKit, libc++
  Concerto defines: GHOSTTY_ENABLED
```

```
swift/
├── LoopflowCore/
│   ├── LoopflowCore.swift          # Constants: lfdDefaultPort=2486, lfdBaseURL, lfdApiBaseURL
│   ├── Models/
│   │   ├── Wave.swift              # Core domain: Wave, Stimulus, WaveStatus, etc.
│   │   ├── WaveRun.swift           # WaveRun, WaveRunStatus
│   │   ├── WaveViewModel.swift     # View-layer enrichment of Wave
│   │   ├── Flow.swift              # Flow, FlowType
│   │   ├── Step.swift              # Step, StepConfig, StepRun
│   │   ├── PullRequest.swift       # PullRequest, PRState
│   │   ├── AppPreferences.swift    # TerminalApp, IDEApp enums
│   │   ├── AppearanceMode.swift    # system/light/dark
│   │   └── StatusColors.swift      # Color extensions for status
│   └── Services/
│       ├── LocalWaveService.swift  # HTTP client for lfd daemon
│       ├── LocalEventService.swift # WebSocket client for live events
│       ├── WaveServiceProtocol.swift # Protocol + WaveFlowsResult
│       ├── AuthService.swift       # OAuth via loopflow.studio, keychain JWT
│       ├── AuthState.swift         # Observable auth state with auto-refresh
│       ├── AuthError.swift         # Auth error enum
│       ├── TokenProvider.swift     # Protocol: NoAuthProvider, KeychainTokenProvider
│       ├── LoggingService.swift    # File logging to ~/Library/Logs/Concerto/
│       └── NotificationService.swift # macOS user notifications
├── Concerto/
│   ├── ConcertoApp.swift           # @main App entry point
│   ├── BrandColors.swift           # LoopflowPalette, Color extensions
│   ├── DesignSystem.swift          # Spacing, Typography, ButtonStyles
│   ├── Flags.swift                 # Feature flags (beta)
│   ├── ScriptCommands.swift        # Apple Script / automation
│   ├── Models/
│   │   └── RecentRepo.swift
│   ├── State/
│   │   ├── RepoState.swift         # Primary app state (@Observable)
│   │   ├── WaveStore.swift         # Dictionary-keyed wave storage
│   │   ├── RunStore.swift          # Cached runs by wave ID
│   │   └── OutputBuffer.swift      # Agent output buffering
│   ├── Services/
│   │   ├── Ghostty/
│   │   │   ├── GhosttyManager.swift     # Singleton wrapping libghostty C API
│   │   │   ├── GhosttyTerminalView.swift # SwiftUI/AppKit Metal terminal view
│   │   │   └── GhosttyTypes.swift       # TerminalStatus, GhosttySession
│   │   ├── SetupService.swift      # Dependency checking/installation
│   │   ├── TerminalLauncher.swift  # Launch terminals/IDEs at paths
│   │   ├── NameGenerator.swift     # Random wave name generation
│   │   ├── RecentsService.swift    # Recent repos via UserDefaults
│   │   ├── RecentAreasService.swift # Recent areas per repo
│   │   ├── SnapshotService.swift   # Window capture for screenshots
│   │   └── AppIconProvider.swift   # App icons for terminal/IDE
│   └── Views/
│       ├── ContentView.swift       # NavigationSplitView: sidebar + detail
│       ├── WaveSidebar.swift       # Wave list with groups
│       ├── WaveRow.swift           # Sidebar row per wave
│       ├── WaveDetailPanel.swift   # Two-tab: Current + Runs
│       ├── WaveRunsTab.swift       # Historical runs list
│       ├── WelcomeWindow.swift     # Initial repo picker
│       ├── RepoWindow.swift        # Window wrapper per repository
│       ├── CommandPalette.swift    # Cmd+K command overlay
│       ├── QuickExperimentView.swift # Quick one-off step launcher
│       ├── LiveOutput.swift        # Streaming agent output display
│       ├── FlowProgressPills.swift # Step progress indicators
│       ├── IterationTimeline.swift # Iteration history visualization
│       ├── NextActionsBar.swift    # Post-run action buttons
│       ├── WaitingStateCard.swift  # Waiting state display
│       ├── InteractiveSessionView.swift # Embedded terminal panel
│       ├── EmbeddedTerminalPanel.swift  # Terminal container
│       ├── StepRunner.swift        # Step execution view
│       ├── SetupView.swift         # Dependency setup wizard
│       ├── DiagnosticsView.swift   # Debug/logs view
│       ├── ScreenshotWindow.swift  # Screenshot automation
│       ├── TerminalTestWindow.swift # Ghostty test window
│       ├── ThemePreview.swift      # Color palette preview
│       ├── AreaTypeahead.swift     # Area input with suggestions
│       ├── DirectionTypeahead.swift # Direction input
│       ├── FlowTypeahead.swift     # Flow/step picker
│       └── TypeaheadComponents.swift # Shared typeahead widgets
├── Symphonia/                      # Minimal secondary app
├── ConcertoTests/                  # Unit tests (ViewInspector)
├── ConcertoUITests/                # UI/screenshot tests
└── SymphoniaTests/
```

---

## Core Data Structures

### Wave (LoopflowCore/Models/Wave.swift)

The central domain model. An autonomous AI coding wave.

```swift
public struct Wave: Sendable, Identifiable, Hashable {
    public let id: String
    public var name: String
    public var repo: String               // filesystem path
    public var flow: String               // flow name (e.g. "ship", "debug")
    public var direction: [String]        // persona/role (e.g. "product-engineer")
    public var area: [String]             // code area (e.g. "src/auth", ".")
    public var stimuli: [Stimulus]        // trigger rules
    public var status: WaveStatus         // idle|running|waiting|failed|paused
    public var iteration: Int             // current iteration number
    public var localWorktree: String?     // git worktree path
    public var remoteBranch: String?      // remote branch name
    public var commits: [CommitEntry]     // commits on branch
    public var diffStat: String?          // diff summary
    public var flowSteps: [String]        // ordered step names in flow
    public var openPRCount: Int           // number of open PRs
    public var activeRun: WaveRun?        // currently executing run
    public var createdAt: Date?
}
```

### Stimulus

```swift
public struct Stimulus: Sendable, Hashable, Codable, Identifiable {
    public var id: String
    public var kind: Kind                 // .loop | .watch | .cron
    public var enabled: Bool
    public var cron: String?              // cron expression (only for .cron)

    public enum Kind: String, Sendable, Codable, CaseIterable {
        case loop   // continuous
        case watch  // on file change
        case cron   // scheduled
    }
}
```

### WaveStatus / WaveRunStatus

```swift
public enum WaveStatus: String, Sendable, Codable {
    case idle, running, waiting, failed, paused
}

public enum WaveRunStatus: String, Sendable, Codable {
    case pending, running, waiting, completed, failed, cancelled
}
```

### WaveRun (LoopflowCore/Models/WaveRun.swift)

A single execution of a wave.

```swift
public struct WaveRun: Sendable, Identifiable, Hashable {
    public let id: String
    public let waveId: String?
    public let flow: String               // flow name
    public let area: String               // area (flattened to string)
    public let repo: String
    public let direction: [String]

    public var status: WaveRunStatus
    public var iteration: Int
    public var stepIndex: Int             // current step in flow

    public var worktree: String?          // git worktree path
    public var branch: String?
    public var currentStep: String?       // name of currently executing step
    public var error: String?             // error message on failure
    public var pr: PullRequest?           // PR created by this run

    public var startedAt: Date?
    public var endedAt: Date?
    public var createdAt: Date

    // Computed
    var duration: String?                 // "3m45s" format
    var relativeTime: String              // "2h ago" format
}
```

### WaveViewModel (LoopflowCore/Models/WaveViewModel.swift)

Enriched view model wrapping `Wave` with git state, PR state, and display helpers.

```swift
public struct WaveViewModel: Sendable, Identifiable, Hashable {
    public var api: Wave                  // underlying API model

    // Git state
    public var worktreePath: String?
    public var branch: String?
    public var isDirty: Bool
    public var isRebasing: Bool
    public var isMerging: Bool
    public var hasDiff: Bool
    public var aheadMain: Int
    public var behindMain: Int
    public var aheadRemote: Int
    public var behindRemote: Int

    // PR state
    public var prURL: URL?
    public var prNumber: Int?
    public var prState: PRState?

    // Run history
    public var recentSteps: [StepRun]

    // Configuration
    public var prLimit: Int               // max open PRs (default 5)
    public var mergeMode: MergeMode       // .pr | .land

    // Runtime
    public var pid: Int?
    public var lastMainSha: String?
    public var waitingReason: WaitingReason?
    public var runStartedAt: Date?

    // Delegated getters/setters to api: name, repo, flow, direction, area,
    // stimuli, status, iteration, activeRun, createdAt, commits, diffStat,
    // flowSteps, openPRCount

    // Computed display helpers
    var displayName: String               // name or "area · flow"
    var detailText: String                // "area · flow · stimulus"
    var statusText: String                // "Running", "Idle", etc.
    var statusIndicator: (icon, color)    // SF Symbol + Color
    var pendingPR: (number, url)?         // open PR if any
    var lastActivityAt: Date?
    var lastActivityDescription: String?
}
```

### Flow / Step / StepRun

```swift
public enum FlowType: String, Sendable, Codable { case flow, step }

public struct Flow: Sendable, Codable, Identifiable, Equatable {
    public var name: String
    public var steps: [Step]
    public var type: FlowType
}

public struct StepConfig: Sendable, Codable, Equatable {
    public var model: String?             // AI model override
    public var direction: String?         // persona override
    public var context: [String]?         // additional context files
}

public struct Step: Sendable, Codable, Equatable, Identifiable {
    public var prompt: String             // step/prompt name
    public var config: StepConfig?
}

public struct StepRun: Sendable, Identifiable, Codable, Hashable {
    public let id: String
    public let step: String               // step name
    public let repo: String
    public let worktree: String
    public let status: String             // "running"|"completed"|"error"|"waiting"
    public let startedAt: Date
    public let endedAt: Date?
    public let model: String              // AI model used
    public let runMode: String            // execution mode
}
```

### PullRequest

```swift
public enum PRState: String, Sendable, Codable {
    case open, merged, closed, draft
}

public struct PullRequest: Sendable, Hashable, Codable {
    public let url: URL
    public let number: Int?
    public let state: PRState?
    public let title: String?
    public let branch: String?
}
```

### Other Models

```swift
public enum MergeMode: String, Sendable, Codable { case pr, land }

public enum WaitingReason: Sendable, Hashable {
    case prLimitReached(open: Int, limit: Int)
}

public struct InteractiveSession: Sendable, Identifiable {
    public let id: String
    public let waveId: String
    public let step: String
    public let worktreePath: String
    public let prompt: String?
    public let startedAt: Date
    var command: String                   // "lf <step> [prompt]"
}

public struct CommitEntry: Sendable, Hashable, Identifiable {
    public let sha: String
    public let message: String
}

public enum TerminalApp: String, CaseIterable { case warp, iterm, terminal, kitty }
public enum IDEApp: String, CaseIterable { case cursor, vscode, zed }
public enum AppearanceMode: String, CaseIterable { case system, light, dark }
```

---

## Services

### WaveServiceProtocol (LoopflowCore/Services/WaveServiceProtocol.swift)

Protocol defining all wave operations. Implemented by `LocalWaveService`, mockable for tests.

```swift
public protocol WaveServiceProtocol: Sendable {
    func listWaves(repo: URL) async throws -> [Wave]
    func getWave(_ id: String) async throws -> Wave
    func createWave(name: String, repo: URL) async throws -> Wave
    func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave
    func deleteWave(_ id: String) async throws
    func cloneWave(_ id: String, name: String?) async throws -> Wave
    func run(_ id: String, overrides: RunOverrides?) async throws
    func addStimulus(_ waveId: String, kind: Stimulus.Kind, cron: String?) async throws -> Stimulus
    func removeStimulus(_ waveId: String, stimulusId: String) async throws
    func stop(_ id: String) async throws
    func landWave(_ id: String) async throws
    func nextWave(_ id: String) async throws -> String
    func listFlowsAndDirections(repo: URL) async throws -> WaveFlowsResult
}

public struct WaveFlowsResult: Sendable {
    public var flows: [Flow]
    public var directions: [String]
}

public struct WaveConfigUpdate: Sendable {
    public var name: String?
    public var area: [String]?
    public var direction: [String]?
    public var flow: String?
    public var status: WaveStatus?
}

public struct RunOverrides: Sendable {
    public var area: [String]?
    public var direction: [String]?
    public var flow: String?
}
```

### LocalWaveService (LoopflowCore/Services/LocalWaveService.swift)

HTTP client for lfd daemon at `http://127.0.0.1:2486/v0/`.

- Two URLSession instances: default (3s timeout) and `longSession` (30s timeout for git ops)
- JSON parsing via `parseWaveFromJSON()` / `parseWaveRunFromJSON()` static methods
- Status normalization: `"error" -> "failed"`, `"completed" -> "idle"`
- String list normalization handles both `["a","b"]` and `"[\"a\",\"b\"]"` formats

Additional endpoints beyond protocol:

```swift
func connectLfd() async throws                    // runs "lfd install"
func checkAvailability() async -> Bool             // GET /status
func connect(_ id: String) async throws -> ConnectionInfo  // POST /v0/waves/{id}/connect
func listWaveRuns(waveId:repo:limit:) async throws -> [WaveRun]
func combinePRs(_ id: String) async throws -> CombinePRsResult
func streamOutput(waveId: String) -> AsyncThrowingStream<String, Error>  // GET /v0/waves/{id}/logs

public struct ConnectionInfo: Sendable {
    public let worktree: String
    public let step: String
    public let agentId: String
    public let promptFile: String
    public let waveRunId: String?
    public let stepIndex: Int
}

public struct CombinePRsResult: Sendable {
    public let newPRUrl: String?
    public let closedPRs: [Int]
}

public enum WaveServiceError: LocalizedError {
    case commandFailed(String)
}
```

### LocalEventService (LoopflowCore/Services/LocalEventService.swift)

WebSocket client for live events at `ws://127.0.0.1:2486/ws`.

```swift
public actor LocalEventService {
    func subscribe(
        onEvent: @Sendable (LFDEvent) -> Void,
        onConnectionChange: @Sendable (Bool) -> Void
    ) async
    func disconnect() async
    var isConnected: Bool
}

public enum LFDEvent: Sendable {
    case connected(ConnectedEvent)        // initial state with all waves
    case wave(WaveEvent)                  // CRUD + lifecycle
    case worktree(WorktreeEvent)          // worktree updates
    case agentStarted(AgentStartedEvent)  // agent execution started
    case agentEnded(AgentEndedEvent)      // agent execution ended
    case output(OutputEvent)              // live output lines
}

public enum WaveEventType: String, Sendable {
    case created, updated, deleted, started, stopped, waiting
}

// Wire format: "wave_created" -> .created (prefix "wave_" stripped)
// Reconnect: exponential backoff — 1s for first 10 attempts, then 5s
```

### AuthService (LoopflowCore/Services/AuthService.swift)

OAuth authentication via `loopflow.studio`.

```swift
public final class AuthService: NSObject {
    @MainActor func signIn() async throws -> String    // ASWebAuthenticationSession
    func signOut() throws
    func currentToken() -> String?                      // from keychain
    func tokenExpiresAt() -> Date?                      // JWT exp claim
    func refreshToken() async throws -> String          // POST /auth/refresh
}
// Keychain: service="studio.loopflow.auth", account="jwt"
// Callback scheme: "loopflow://auth/callback"
```

### AuthState (LoopflowCore/Services/AuthState.swift)

```swift
@MainActor @Observable
public final class AuthState {
    public private(set) var token: String?
    public private(set) var isLoading: Bool
    public private(set) var error: AuthError?
    public var isAuthenticated: Bool
    public var isExpired: Bool
    public var needsRefresh: Bool          // within 24h of expiry
    func signIn() async
    func signOut()
    // Background refresh monitor checks hourly
}
```

### AuthError

```swift
public enum AuthError: Error, Sendable {
    case noCallback
    case invalidCallback
    case notAuthenticated
    case tokenExpired
    case sessionFailed
    case refreshFailed(String)
    case keychainWrite(OSStatus)
    case keychainDelete(OSStatus)
    case unknown(Error)
}
```

### LoggingService

```swift
public enum LoggingService {
    enum Category: String { case worktrees, lfd, general, ui, model }
    static func append(_ message: String, category: Category)
    static func ui(_ message: String)
    static func model(_ message: String)
    static func lfd(_ message: String)
    static func read(category:) -> String
    static func logDirectory() -> URL     // ~/Library/Logs/Concerto/
}
```

### NotificationService

```swift
public final class NotificationService: NSObject, UNUserNotificationCenterDelegate {
    static let shared: NotificationService
    func requestAuthorization() async throws
    func notifyNeedsInteractive(waveId:waveName:step:)   // waiting for input
    func notifyError(waveId:waveName:message:)            // wave failed
    func notifyPRReady(waveId:waveName:prNumber:)         // PR created
}
// Tapping notification posts Notification.Name.selectWave
```

---

## App State (Concerto/State/)

### RepoState (primary state container)

```swift
@MainActor @Observable
final class RepoState {
    var currentRepo: URL?
    var flows: [Flow]
    var availableDirections: [String]

    let waveStore: WaveStore
    let runStore: RunStore
    var waves: [WaveViewModel]             // derived from waveStore.ordered
    var waveGroups: WaveGroups              // derived from waveStore.groups

    var selectedWaveId: String?
    var selectedWave: WaveViewModel?        // computed from selectedWaveId
    private(set) var inFlightActions: Set<String>

    var isLoading: Bool
    var errorMessage: String?
    var lfdConnected: Bool

    // Lifecycle
    func openRepo(_ url: URL, outputBuffer: OutputBuffer, skipBackgroundRefresh: Bool) async
    func startEventSubscription(outputBuffer: OutputBuffer)
    func connectLfd(outputBuffer: OutputBuffer) async throws

    // Wave CRUD
    func createWave(name: String) async throws
    func deleteWave(_ wave: WaveViewModel) async throws
    func cloneWave(_ wave: WaveViewModel) async throws -> WaveViewModel
    func renameWave(_ wave: WaveViewModel, to: String) async throws
    func updateWave(_ wave: WaveViewModel, area:direction:flow:status:) async throws

    // Wave actions
    func runWave(wave:area:direction:flow:) async throws
    func stopWave(_ wave: WaveViewModel) async throws
    func landWave(_ wave: WaveViewModel) async throws
    func nextWave(_ wave: WaveViewModel) async throws

    // Stimulus
    func addStimulus(wave:kind:cron:) async throws
    func removeStimulus(wave:stimulusId:) async throws

    // Runs
    func loadRuns(for waveId: String)
    func combinePRs(_ waveId: String) async throws -> CombinePRsResult

    // Flows
    func refreshFlowsAsync() async
    func refreshWaves() async

    // UI test support
    static func uiTestMode() -> UITestMode?
    func configureForUITest(_ mode: UITestMode, repoURL: URL)
    func configureMockWaves()
}
```

**Optimistic update pattern** (used by all mutations):

```swift
private func optimistic(_ id: String, mutation: (inout WaveViewModel) -> Void, apiCall: () async throws -> Void) async throws {
    let snapshot = waveStore.applyOptimistic(id, mutation)
    do {
        try await apiCall()
        waveStore.commitMutation(id)
    } catch {
        if let snapshot { waveStore.rollback(snapshot) }
        throw error
    }
}
```

### WaveStore

Dictionary-keyed wave storage with derived grouping and optimistic mutation support.

```swift
@MainActor @Observable
final class WaveStore {
    private(set) var waves: [String: WaveViewModel]
    private(set) var ordered: [WaveViewModel]           // recomputed on change
    private(set) var groups: WaveGroups                  // recomputed on change

    var onStatusChange: ((WaveViewModel, WaveStatus?, WaveStatus) -> Void)?

    func set(_ wave: WaveViewModel)                     // skips if pending mutation
    func setAll(_ newWaves: [WaveViewModel])             // bulk replace (preserves pending)
    func remove(_ id: String) -> WaveViewModel?
    func removeAll()
    func wave(for id: String) -> WaveViewModel?

    // Optimistic mutations
    func applyOptimistic(_ id: String, _ mutation: (inout WaveViewModel) -> Void) -> WaveViewModel?
    func commitMutation(_ id: String)
    func rollback(_ snapshot: WaveViewModel)

    // Pending create/delete
    func insertPending(_ wave: WaveViewModel)
    func replacePending(_ pendingId: String, with: WaveViewModel)
    func removePending(_ id: String)
    func applyDelete(_ id: String)
}

struct WaveGroups {
    let blocked: [WaveViewModel]          // status == .failed
    let pr: [WaveViewModel]               // has open PRs (non-failed)
    let recentActivity: [WaveViewModel]   // activity within 1 hour (max 5)
    let active: [WaveViewModel]           // running/waiting, not in recent
    let idle: [WaveViewModel]             // idle, not in recent
    var attentionCount: Int               // blocked + pr count
    var openPRCount: Int                  // total open PRs across pr group
    var allInOrder: [WaveViewModel]       // concatenated: blocked + pr + recent + active + idle
}
```

### RunStore

```swift
@MainActor @Observable
final class RunStore {
    private(set) var runs: [String: [WaveRun]]    // keyed by wave ID
    func setRuns(for waveId: String, _ runs: [WaveRun])  // max 50 per wave
    func runs(for waveId: String) -> [WaveRun]
    func clear(for waveId: String)
}
```

### OutputBuffer

```swift
@MainActor @Observable
final class OutputBuffer {
    var interactiveSession: InteractiveSession?

    func appendOutput(waveId:text:timestamp:)       // from WebSocket (skipped if streaming)
    func output(for waveId: String) -> [OutputLine]
    func clearOutput(for waveId: String)

    func startStreaming(waveId: String)              // HTTP replay+follow endpoint
    func stopStreaming(waveId: String)
    func recentOutput(for waveId:maxLength:) -> String?

    func launchInteractiveSession(waveId:step:worktreePath:prompt:)
    func endInteractiveSession()
    func hasActiveSession(for waveId: String) -> Bool
}

struct OutputLine: Identifiable {
    let id: UUID
    let text: String
    let timestamp: Date
}
// Max 2000 lines per wave. Generation counter prevents stale stream cleanup.
```

---

## Design System (Concerto/)

### Brand Colors (BrandColors.swift)

```swift
Color.loopflowBurgundy = #722F37
Color.loopflowBurgundyHover = #8B3D47
Color.loopflowCream = #FAF8F5

struct LoopflowPalette {
    let background, surface, surfaceMuted, border: Color
    let text, textSecondary, accent, accentHover: Color

    static let light     // cream-based
    static let dark      // slate-based
    static let deepWine  // burgundy-based
}
// Injected via EnvironmentValues.palette
```

### Status Colors (LoopflowCore/Models/StatusColors.swift)

```swift
Color.statusSuccess = #2D6A4F    // green
Color.statusError   = #B45309    // amber
Color.statusWarning = #B0812A    // gold
Color.statusInfo    = #0AB3CC    // cyan
Color.statusNeutral = #8B8B8B    // gray
```

### Spacing / Layout (DesignSystem.swift)

```swift
enum Spacing {
    static let xxs: 2, xs: 4, sm: 8, md: 12, lg: 16, xl: 20, xxl: 24, xxxl: 32
}
enum HitTarget { static let minimum: 24, comfortable: 32, touch: 44 }
enum ZIndex { static let base: 0, dropdown: 100, modal: 200, toast: 300, tooltip: 400 }
enum CornerRadius { static let sm: 4, md: 8, lg: 12, xl: 16, full: 9999 }
```

### Typography

```swift
enum Typography {
    static let serifFamily = "Cormorant Garamond"   // headlines
    static let sansFamily = "Lato"                  // body/UI
    static let monoFamily = "JetBrains Mono"        // code
    static func heroTitle(_ size: CGFloat = 32) -> Font
    static func sectionTitle(_ size: CGFloat = 20) -> Font
    static func body(_ size: CGFloat = 14) -> Font
    static func caption(_ size: CGFloat = 12) -> Font
    static func code(_ size: CGFloat = 13) -> Font
}
```

### Button Styles

```swift
struct DarkButtonStyle: ButtonStyle        // burgundy bg, cream text
struct GhostButtonStyle: ButtonStyle       // transparent, accent text
struct DestructiveButtonStyle: ButtonStyle  // error-colored border
```

---

## Ghostty Terminal Integration (Concerto/Services/Ghostty/)

Embedded terminal using libghostty C API via xcframework.

```swift
@MainActor
final class GhosttyManager: ObservableObject {
    enum State { case uninitialized, initializing, ready, failed(String) }
    static let shared: GhosttyManager

    func initialize()                                          // ghostty_init + ghostty_app_new
    func tick()                                                // ghostty_app_tick
    func createSurface(workingDirectory:command:view:) -> ghostty_surface_t?
    func destroySurface(_ surface: ghostty_surface_t)
    func registerActiveSession(_ surface:sessionId:)
    func destroyActiveSession()
    func sendText(_ text: String)
    var onSessionClosed: (() -> Void)?
}
// Loopflow theme: cream (#F5E6D3) on burgundy (#4A1A2C)
// Stub implementation when GHOSTTY_ENABLED not defined

enum TerminalStatus: Equatable {
    case initializing, running, completed(exitCode: Int32), failed(error: String)
}
struct GhosttySession: Identifiable {
    let id: String, worktree: String, command: [String]
    var status: TerminalStatus
    var surface: ghostty_surface_t?    // when GhosttyKit available
}
```

---

## Concerto Services

### SetupService

```swift
struct SetupService {
    struct DependencyStatus { var lfInstalled: Bool, lfPath: String? }
    func checkDependencies() -> DependencyStatus
    func install() async throws          // installs uv -> loopflow -> node -> claude
    func ensureDaemonRunning() async throws  // lfd install + launchctl
}
// Searches: ~/.local/bin, /opt/homebrew/bin, /usr/local/bin, /usr/bin, ~/.cargo/bin
// Logs to ~/.lf/logs/concerto-setup.log
```

### TerminalLauncher

Launches external terminals and IDEs at file paths. Supports Warp, iTerm, Terminal, Kitty, Cursor, VS Code, Zed.

### NameGenerator

Random name generation: `{magical}-{musical}` (e.g. "crystal-melody"). Words drawn from curated lists.

---

## App Entry Point (ConcertoApp.swift)

```swift
@main struct ConcertoApp: App {
    // Registers bundled fonts on init (Cormorant Garamond, Lato, JetBrains Mono)
    // Requests notification authorization
    // Resolves palette from AppearanceMode (system/light/dark)

    // Scenes:
    //   WindowGroup (default): WelcomeWindow or ScreenshotWindow or UITest RepoWindow
    //   WindowGroup(id: "repo"): RepoWindow per repository (900x700)
    //   Window("Terminal Test"): GhosttyTerminalView test

    // Commands:
    //   Beta Features toggle
    //   Appearance picker (system/light/dark)
    //   Snapshot for Review (Cmd+4)
    //   Command Palette (Cmd+K)
    //   Terminal Test (Cmd+Shift+T)
}
```

---

## Key Patterns

### Event-Driven Architecture
- `LocalEventService` (WebSocket) pushes `LFDEvent`s
- `RepoState.startEventSubscription()` processes events, updates `WaveStore`
- `NotificationCenter` for cross-view actions: `.toggleCommandPalette`, `.newWaveRequested`, `.selectWave`

### Optimistic Updates
- `WaveStore.applyOptimistic()` records snapshot, marks mutation pending
- `WaveStore.commitMutation()` clears pending flag
- `WaveStore.rollback()` restores snapshot
- Pending mutations block WebSocket/API event updates for that wave
- Create/delete use `insertPending` / `replacePending` / `removePending` / `applyDelete`

### State Management
- `@Observable` macro on state classes
- `@MainActor` isolation for UI state
- `@Environment(RepoState.self)` injection in views
- `@Environment(OutputBuffer.self)` for output
- `@Environment(\.palette)` for theming

### HTTP/WS Communication
- lfd daemon at `127.0.0.1:2486`
- REST API: `GET/POST/PATCH/DELETE /v0/waves/{id}/*`
- WebSocket: `ws://127.0.0.1:2486/ws`
- Output streaming: `GET /v0/waves/{id}/logs` (HTTP streaming, lines)
- Two timeout tiers: 3s (fast-fail) vs 30s (git operations)

### Testing
- `ViewInspector` for SwiftUI view tests
- UI test modes via `-ui-test-mode` arg or `CONCERTO_UI_TEST_MODE` env:
  `empty-workspaces`, `sample-workspaces`, `mock-waves`
- Screenshot mode via `--snapshot` arg with `--repo`, `--size`, `--select`, `--mock-loops`, `--tab`

### Build
```bash
swift build --package-path swift                   # build all
swift test --package-path swift                    # run tests
swift build --package-path swift --product Concerto  # build app only
```
