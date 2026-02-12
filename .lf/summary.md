# Swift Codebase Summary

Package: `LoopflowSwift` — Swift 6.0, macOS 15+

```
swift/
├── Package.swift
├── LoopflowCore/          # Shared library — models + services
│   ├── LoopflowCore.swift # Constants: lfdDefaultPort=2486, lfdBaseURL, lfdApiBaseURL
│   ├── Models/
│   │   ├── Flow.swift
│   │   ├── Step.swift
│   │   ├── PullRequest.swift
│   │   ├── AppPreferences.swift
│   │   └── AppearanceMode.swift
│   └── Services/
│       ├── LocalWaveService.swift   # HTTP client to lfd daemon
│       ├── LocalEventService.swift  # WebSocket subscriber
│       ├── WaveServiceProtocol.swift
│       ├── AuthService.swift        # OAuth + JWT keychain
│       ├── AuthState.swift          # Observable auth state
│       ├── AuthError.swift
│       ├── TokenProvider.swift
│       ├── NotificationService.swift
│       └── LoggingService.swift
├── Concerto/              # macOS app — wave management UI
│   ├── ConcertoApp.swift  # Entry point, window groups, fonts
│   ├── Flags.swift        # Feature flags (beta)
│   ├── ScriptCommands.swift # AppleScript handlers
│   ├── Models/
│   │   └── RecentRepo.swift
│   ├── Services/
│   │   ├── TerminalLauncher.swift
│   │   ├── SetupService.swift
│   │   ├── SnapshotService.swift
│   │   ├── RecentsService.swift
│   │   ├── RecentAreasService.swift
│   │   ├── NameGenerator.swift
│   │   ├── AppIconProvider.swift
│   │   └── Ghostty/
│   │       ├── GhosttyManager.swift
│   │       └── GhosttyTypes.swift
│   ├── State/
│   │   ├── RepoState.swift    # Main observable state
│   │   ├── WaveStore.swift    # Dictionary-keyed wave state
│   │   ├── OutputBuffer.swift # Agent output buffering
│   │   └── RunStore.swift     # Wave run cache
│   └── Views/
│       ├── RepoWindow.swift
│       ├── ContentView.swift           # NavigationSplitView layout
│       ├── WaveSidebar.swift           # Grouped wave list
│       ├── WaveRow.swift               # Sidebar row
│       ├── WaveDetailPanel.swift       # Detail: config, output, actions
│       ├── StepRunner.swift            # Step execution config
│       ├── InteractiveSessionView.swift # Embedded Ghostty terminal
│       ├── WaveRunsTab.swift           # Historical runs + PR management
│       ├── CommandPalette.swift        # ⌘K fuzzy search
│       ├── AreaTypeahead.swift
│       ├── DirectionTypeahead.swift
│       └── FlowTypeahead.swift
├── Symphonia/             # Teams product — placeholder
├── ConcertoTests/
├── ConcertoUITests/
└── SymphoniaTests/
```

## Dependencies

```swift
// Package.swift
.package(url: "https://github.com/nalexn/ViewInspector.git", from: "0.10.0")  // SwiftUI test inspection
.binaryTarget(name: "GhosttyKit", url: "https://bin.loopflow.studio/GhosttyKit-061a0ae.xcframework.zip")

// Concerto linker settings
.linkedFramework("Carbon"), .linkedFramework("QuartzCore"),
.linkedFramework("Metal"), .linkedFramework("IOKit"), .linkedLibrary("c++")
```

Build flag: `GHOSTTY_ENABLED` — conditional compilation for embedded terminal.

---

## LoopflowCore — Models

### Wave (core domain object)

```swift
// Wave.swift
public struct Wave: Sendable, Identifiable, Hashable {
    public let id: String
    public var name: String
    public var repo: String
    public var flow: String
    public var direction: [String]
    public var area: [String]
    public var stimuli: [Stimulus]
    public var status: WaveStatus
    public var iteration: Int
    public var localWorktree: String?
    public var remoteBranch: String?
    public var commits: [CommitEntry]
    public var diffStat: String?
    public var flowSteps: [String]
    public var openPRCount: Int
    public var activeRun: WaveRun?
    public var createdAt: Date?
}

public enum WaveStatus: String, Sendable, Codable {
    case idle, running, waiting, failed, paused
}

public struct Stimulus: Sendable, Hashable, Codable, Identifiable {
    public enum Kind: String, Sendable, Codable, CaseIterable {
        case loop   // Continuous execution
        case watch  // On file change
        case cron   // Scheduled
    }
    public var id: String
    public var kind: Kind
    public var cron: String?
}

public struct CommitEntry: Sendable, Hashable, Identifiable {
    public let sha: String
    public let message: String
}

public struct InteractiveSession: Sendable, Identifiable {
    public let id: String
    public let waveId: String
    public let step: String
    public let worktreePath: String
    public let prompt: String?
    public let startedAt: Date
}
```

### WaveRun (single execution of a wave)

```swift
// WaveRun.swift
public struct WaveRun: Sendable, Identifiable, Hashable {
    public let id: String
    public let waveId: String?
    public let flow: String
    public let area: String
    public let repo: String
    public let direction: [String]
    public var status: WaveRunStatus
    public var iteration: Int
    public var stepIndex: Int
    public var worktree: String?
    public var branch: String?
    public var currentStep: String?
    public var error: String?
    public var pr: PullRequest?
    public var startedAt: Date?
    public var endedAt: Date?
    public var createdAt: Date
}

public enum WaveRunStatus: String, Sendable, Codable {
    case pending, running, waiting, completed, failed, cancelled
}
```

### WaveViewModel (enriched wave with git/UI state)

```swift
// WaveViewModel.swift
public struct WaveViewModel: Sendable, Identifiable, Hashable {
    public var api: Wave                     // Core API model
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
    public var prURL: URL?
    public var prNumber: Int?
    public var prState: PRState?
    public var recentSteps: [StepRun]
    public var prLimit: Int
    public var mergeMode: MergeMode
    public var pid: Int?
    public var lastMainSha: String?
    public var waitingReason: WaitingReason?
    public var runStartedAt: Date?

    // Key computed properties
    public var id: String                    // delegates to api.id
    public var displayName: String
    public var areaDisplay: String           // comma-joined areas
    public var directionDisplay: String      // comma-joined directions
    public var statusText: String
    public var iterationText: String         // "iteration N"
    public var detailText: String            // step progress
    public var stimulusText: String          // "loop", "watch", "cron: ..."
    public var hasActiveStimulus: Bool
    public var statusIndicator: (icon: String, color: Color)
    public var pendingPR: (number: Int, url: URL?)?
    public var lastActivityAt: Date?
    public var lastActivityDescription: String?
}
```

### Flow / Step

```swift
// Flow.swift
public enum FlowType: String, Sendable, Codable { case flow, step }

public struct Flow: Sendable, Codable, Identifiable, Equatable {
    public var id: UUID
    public var name: String
    public var steps: [Step]
    public var type: FlowType
}

// Step.swift
public struct StepConfig: Sendable, Codable, Equatable {
    public var model: String?
    public var direction: String?
    public var context: [String]?
    public var isEmpty: Bool  // computed
}

public struct Step: Sendable, Codable, Equatable, Identifiable {
    public var id: UUID
    public var prompt: String
    public var config: StepConfig?
}

public struct StepRun: Sendable, Identifiable, Codable, Hashable {
    public let id: String
    public let step: String
    public let repo: String
    public let worktree: String
    public let status: String      // "running", "completed", "error"
    public let startedAt: Date
    public let endedAt: Date?
    public let model: String
    public let runMode: String
    public var isRunning: Bool     // computed
    public var isCompleted: Bool   // computed
    public var isError: Bool       // computed
}
```

### PullRequest

```swift
// PullRequest.swift
public enum PRState: String, Sendable, Codable {
    case open, merged, closed, draft
    public var displayText: String  // computed
}

public struct PullRequest: Sendable, Hashable, Codable {
    public let url: URL
    public let number: Int?
    public let state: PRState?
    public let title: String?
    public let branch: String?
}
```

### Preferences

```swift
// AppPreferences.swift
public enum TerminalApp: String, Sendable, CaseIterable {
    case warp, iterm, terminal, kitty
    public var displayName: String
}

public enum IDEApp: String, Sendable, CaseIterable {
    case cursor, vscode, zed
    public var displayName: String
}

// AppearanceMode.swift
public enum AppearanceMode: String, Sendable, CaseIterable {
    case system, light, dark
    public var menuTitle: String
    public var colorScheme: ColorScheme?
}
```

---

## LoopflowCore — Services

### LocalWaveService (HTTP client to lfd daemon)

```swift
// LocalWaveService.swift
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

public struct ConnectionInfo: Sendable {
    public let worktree: String
    public let step: String
    public let agentId: String
    public let promptFile: String
    public let waveRunId: String?
    public let stepIndex: Int
}

public struct CollapsePRsResult: Sendable { ... }
public struct AbsorbIntoPRResult: Sendable { ... }

public struct LocalWaveService: WaveServiceProtocol {
    public init(tokenProvider: TokenProvider = NoAuthProvider())

    // CRUD
    func listWaves(repo: URL) async throws -> [Wave]
    func getWave(_ id: String) async throws -> Wave
    func createWave(name: String, repo: URL) async throws -> Wave
    func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave
    func deleteWave(_ id: String) async throws
    func cloneWave(_ id: String, name: String?) async throws -> Wave

    // Actions
    func run(_ id: String, overrides: RunOverrides?) async throws
    func stop(_ id: String) async throws
    func landWave(_ id: String) async throws
    func nextWave(_ id: String) async throws -> String
    func connect(_ id: String) async throws -> ConnectionInfo

    // PR operations
    func collapsePRs(_ id: String) async throws -> CollapsePRsResult
    func absorbIntoPR(_ id: String, prNumber: Int) async throws -> AbsorbIntoPRResult

    // Stimuli
    func addStimulus(_ waveId: String, kind: Stimulus.Kind, cron: String?) async throws -> Stimulus
    func removeStimulus(_ waveId: String, stimulusId: String) async throws

    // Output
    func streamOutput(waveId: String) -> AsyncThrowingStream<String, Error>

    // Metadata
    func listFlowsAndDirections(repo: URL) async throws -> WaveFlowsResult
    func listWaveRuns(waveId: String?, repo: URL?, limit: Int) async throws -> [WaveRun]

    // Setup
    func connectLfd() async throws
    func checkAvailability() async -> Bool
}
```

### WaveServiceProtocol

```swift
// WaveServiceProtocol.swift
public struct WaveFlowsResult: Sendable {
    public var flows: [Flow]
    public var directions: [String]
}

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
```

### LocalEventService (WebSocket)

```swift
// LocalEventService.swift
public enum WaveEventType: String, Sendable {
    case created, updated, deleted, started, stopped, waiting
}

public struct WaveEvent: Sendable {
    public let type: WaveEventType
    public let waveId: String
    public let waveRunId: String?
    public let step: String?
    public let name: String?
    public let wave: Wave?
    public let timestamp: Date
}

public enum LFDEvent: Sendable {
    case connected(ConnectedEvent)
    case wave(WaveEvent)
    case worktree(WorktreeEvent)
    case agentStarted(AgentStartedEvent)
    case agentEnded(AgentEndedEvent)
    case output(OutputEvent)
}

public actor LocalEventService {
    func subscribe(
        onEvent: @escaping @Sendable (LFDEvent) -> Void,
        onConnectionChange: @escaping @Sendable (Bool) -> Void
    ) async
    func disconnect() async
}
```

### Auth

```swift
// AuthError.swift
public enum AuthError: Error, Sendable {
    case noCallback, invalidCallback, notAuthenticated, tokenExpired
    case sessionFailed, refreshFailed(String)
    case keychainWrite(OSStatus), keychainDelete(OSStatus)
    case unknown(Error)
}

// TokenProvider.swift
public protocol TokenProvider: Sendable {
    func token() async throws -> String
}
public struct NoAuthProvider: TokenProvider { ... }           // Returns ""
public final class KeychainTokenProvider: TokenProvider { ... } // Reads from keychain

// AuthService.swift
public final class AuthService: NSObject, @unchecked Sendable {
    func signIn() async throws -> String      // OAuth via ASWebAuthenticationSession
    func signOut() throws                     // Clear keychain
    func currentToken() -> String?
    func tokenExpiresAt() -> Date?
    func refreshToken() async throws -> String
}

// AuthState.swift — Observable wrapper
@MainActor @Observable
public final class AuthState {
    public private(set) var token: String?
    public private(set) var isLoading: Bool
    public private(set) var error: AuthError?
    public var isAuthenticated: Bool  // computed
    public var isExpired: Bool        // computed
    public var needsRefresh: Bool     // computed
    public func signIn() async
    public func signOut()
}
```

### Notifications + Logging

```swift
// NotificationService.swift
public final class NotificationService: NSObject, UNUserNotificationCenterDelegate {
    public static let shared: NotificationService
    func requestAuthorization() async throws
    func notifyNeedsInteractive(waveId: String, waveName: String, step: String)
    func notifyError(waveId: String, waveName: String, message: String)
    func notifyPRReady(waveId: String, waveName: String, prNumber: Int)
}

// LoggingService.swift
public enum LoggingService {
    public enum Category: String { case worktrees, lfd, general, ui, model }
    static func append(_ message: String, category: Category = .worktrees)
    static func ui(_ message: String)
    static func model(_ message: String)
    static func lfd(_ message: String)
    static func read(category: Category = .worktrees) -> String
}
// Log directory: ~/Library/Logs/Concerto/
```

---

## Concerto — State Management

### RepoState (main observable state)

```swift
// State/RepoState.swift — ~600 lines
@MainActor @Observable
final class RepoState {
    // Repository
    var currentRepo: URL?
    var flows: [Flow] = []
    var availableDirections: [String] = []

    // Waves (delegated to stores)
    let waveStore = WaveStore()
    let runStore = RunStore()
    var waves: [WaveViewModel] { waveStore.ordered }
    var waveGroups: WaveGroups { waveStore.groups }

    // Selection
    var selectedWaveId: String?
    var selectedWave: WaveViewModel?   // computed from waveStore

    // Connection
    var lfdConnected: Bool = false
    var isLoading: Bool = false
    var errorMessage: String?

    // Key methods
    func openRepo(_ url: URL, outputBuffer: OutputBuffer, skipBackgroundRefresh: Bool = false) async
    func refreshWaves() async
    func refreshFlowsAsync() async
    func createWave(name: String?, flow: String, area: [String], direction: [String], stimulus: Stimulus?) async
    func deleteWave(_ id: String) async
    func landWave(_ id: String) async
    func nextIteration(_ id: String) async
    func stopWave(_ id: String) async
    func loadRuns(for waveId: String)
}
```

### WaveStore (optimistic mutation store)

```swift
// State/WaveStore.swift — ~185 lines
struct WaveGroups {
    let blocked: [WaveViewModel]       // status == .failed
    let pr: [WaveViewModel]            // has open PRs
    let recentActivity: [WaveViewModel] // active in last hour, max 5
    let active: [WaveViewModel]        // running/waiting (excl recent)
    let idle: [WaveViewModel]          // idle (excl recent)
    var attentionCount: Int            // blocked.count + openPRCount
    var openPRCount: Int
    var allInOrder: [WaveViewModel]
    static let empty: WaveGroups
}

@MainActor @Observable
final class WaveStore {
    private(set) var waves: [String: WaveViewModel]
    private(set) var ordered: [WaveViewModel]
    private(set) var groups: WaveGroups
    var onStatusChange: ((WaveViewModel, WaveStatus?, WaveStatus) -> Void)?

    // Mutations
    func set(_ wave: WaveViewModel)
    func setAll(_ newWaves: [WaveViewModel])
    func remove(_ id: String) -> WaveViewModel?
    func removeAll()
    func wave(for id: String) -> WaveViewModel?

    // Optimistic updates (snapshot → mutate → commit or rollback)
    func applyOptimistic(_ id: String, _ mutation: (inout WaveViewModel) -> Void) -> WaveViewModel?
    func commitMutation(_ id: String)
    func rollback(_ snapshot: WaveViewModel)

    // Pending create/delete
    func insertPending(_ wave: WaveViewModel)
    func replacePending(_ pendingId: String, with wave: WaveViewModel)
    func removePending(_ id: String)
    func applyDelete(_ id: String)
}
```

### OutputBuffer

```swift
// State/OutputBuffer.swift — ~130 lines
struct OutputLine: Identifiable {
    let id: UUID
    let text: String
    let timestamp: Date
}

@MainActor @Observable
final class OutputBuffer {
    var interactiveSession: InteractiveSession?

    func appendOutput(waveId: String, text: String, timestamp: Date)
    func output(for waveId: String) -> [OutputLine]
    func clearOutput(for waveId: String)
    func startStreaming(waveId: String)
    func stopStreaming(waveId: String)
    func recentOutput(for waveId: String, maxLength: Int = 60) -> String?
    func launchInteractiveSession(waveId: String, step: String, worktreePath: String, prompt: String?)
    func endInteractiveSession()
    func hasActiveSession(for waveId: String) -> Bool
}
```

### RunStore

```swift
// State/RunStore.swift
@MainActor @Observable
final class RunStore {
    private(set) var runs: [String: [WaveRun]]
    func setRuns(for waveId: String, _ newRuns: [WaveRun])
    func runs(for waveId: String) -> [WaveRun]
    func clear(for waveId: String)
}
```

---

## Concerto — Services

### TerminalLauncher

```swift
// Services/TerminalLauncher.swift — ~327 lines
struct TerminalLauncher {
    func launchTerminal(_ terminal: TerminalApp, at path: URL, command: String?) throws
    func launchStep(_ step: String, terminal: TerminalApp, at repo: URL) throws
    func openInIDE(_ ide: IDEApp, at path: URL, workspace: String?) throws
    func openInFinder(at path: URL)
    func openURL(_ url: URL)
}
// Uses AppleScript for Warp/iTerm/Terminal, Process for Kitty
// IDE: `cursor`/`code`/`zed` CLI commands
```

### GhosttyManager (embedded terminal)

```swift
// Services/Ghostty/GhosttyManager.swift — ~256 lines
@MainActor
final class GhosttyManager: ObservableObject {
    enum State: Equatable { case uninitialized, initializing, ready, failed(String) }
    @Published private(set) var state: State
    func initialize()
    func tick()
    func createSurface(workingDirectory: String, command: String?, view: NSView) -> ghostty_surface_t?
    func destroySurface(_ surface: ghostty_surface_t)
    func registerActiveSession(_ surface: ghostty_surface_t, sessionId: String)
    func destroyActiveSession()
    func sendText(_ text: String)
}

// Services/Ghostty/GhosttyTypes.swift
enum TerminalStatus: Equatable {
    case initializing, running, completed(exitCode: Int32), failed(error: String)
}
struct GhosttySession: Identifiable {
    let id: String
    let worktree: String
    let command: [String]
    var status: TerminalStatus
    var surface: ghostty_surface_t?  // #if canImport(GhosttyKit)
}
```

### Other Services

```swift
// Services/SetupService.swift — ~223 lines
struct SetupService {
    struct DependencyStatus { var lfInstalled: Bool; var lfPath: String? }
    func checkDependencies() -> DependencyStatus
    func install() async throws
    func ensureDaemonRunning() async throws
}

// Services/SnapshotService.swift
@MainActor struct SnapshotService {
    func snapshotKeyWindow() throws -> URL
    func snapshotKeyWindow(to outputPath: String) throws -> URL
    func snapshotWindow(_ window: NSWindow, to outputURL: URL) throws -> URL
}

// Services/RecentsService.swift
@Observable final class RecentsService {
    private(set) var recentRepos: [RecentRepo]
    func addRecent(_ url: URL)
    func removeRecent(_ url: URL)
    func clearAll()
}

// Services/RecentAreasService.swift — per-repo area history in UserDefaults
struct RecentAreasService {
    func recentAreas(for repoURL: URL) -> [String]
    func addRecentArea(_ area: String, for repoURL: URL)
    func clearRecentAreas(for repoURL: URL)
}

// Services/NameGenerator.swift
enum NameGenerator {
    static func generate() -> String  // e.g. "aurora-aria"
}

// Services/AppIconProvider.swift
struct AppIconProvider {
    static func icon(for app: AppIdentifier) -> NSImage?
    static func iconImage(for app: AppIdentifier, size: CGFloat = 24) -> Image
}
enum AppIdentifier {
    case cursor, warp, vscode, iterm, terminal, zed, kitty, github
    init(ide: IDEApp)
    init(terminal: TerminalApp)
    var bundleIdentifier: String?
    var appName: String
    var fallbackSystemImage: String
}
```

### Models

```swift
// Models/RecentRepo.swift
struct RecentRepo: Codable, Identifiable {
    let path: String
    let lastOpened: Date
    var id: String { path }
    var url: URL
    var displayName: String
    var exists: Bool
}
```

---

## Concerto — Views

### Layout hierarchy

```
ConcertoApp
└── RepoWindow(repoURL:)
    ├── SetupView (if lf not installed)
    └── ContentView
        └── NavigationSplitView
            ├── sidebar: WaveSidebar
            │   └── ForEach(waveGroups) → WaveRow
            ├── detail:
            │   ├── WaveDetailPanel(wave:)  (when wave selected)
            │   │   ├── .current tab → StepRunner + commit log + live output
            │   │   └── .runs tab → WaveRunsTab
            │   └── QuickExperimentDetailView  (when no selection)
            └── overlay: CommandPalette (⌘K)
```

### Key Views

```swift
// Views/ContentView.swift
struct ContentView: View {
    @Environment(RepoState.self) var repoState
    @Environment(OutputBuffer.self) var outputBuffer
    // NavigationSplitView { WaveSidebar() } detail: { ... }
    // CommandPalette overlay on ⌘K
}

// Views/WaveSidebar.swift — ~395 lines
// Groups: Blocked, PR, Recent Activity, Active, Idle
// Search/filter, new wave button, attention badge (blocked + PR count)

// Views/WaveRow.swift — ~310 lines
struct WaveRow: View {
    let wave: WaveViewModel
    let isSelected: Bool
    var isKeyboardFocused: Bool = false
    // Displays: name, area, iteration, PR badge, flow, activity
    // Inline name editing on click
}

// Views/WaveDetailPanel.swift — ~795 lines
struct WaveDetailPanel: View {
    let wave: WaveViewModel
    @State private var selectedTab: DetailTab = .current
    enum DetailTab { case current, runs }
    // current: StepRunner + commit log + live output + actions (run/stop/land/next)
    // runs: WaveRunsTab with historical runs + PR collapse/absorb
}

// Views/StepRunner.swift — ~320 lines
// Configure: flow, area, direction
// Run single step or full flow, interactive vs. autonomous mode

// Views/InteractiveSessionView.swift — ~160 lines
// Embedded Ghostty terminal for interactive wave sessions

// Views/WaveRunsTab.swift — ~320 lines
// Historical run list + PR management (collapse, absorb)

// Views/CommandPalette.swift — ~230 lines
struct PaletteAction { let title: String; let icon: String; let shortcut: String?; let action: () -> Void }
struct CommandPalette: View { @Binding var isPresented: Bool; let actions: [PaletteAction] }
// Fuzzy search, arrow key navigation, ⌘K toggle
```

### Typeahead Components

```swift
// Views/AreaTypeahead.swift — file/directory picker with suggestions
// Views/DirectionTypeahead.swift — direction suggestions from availableDirections
// Views/FlowTypeahead.swift — flow/step picker from loaded flows
```

---

## Concerto — Design System

```swift
// Views/DesignSystem.swift
enum Spacing {
    static let xxs: CGFloat = 2;  static let xs: CGFloat = 4
    static let sm: CGFloat = 8;   static let md: CGFloat = 12
    static let lg: CGFloat = 16;  static let xl: CGFloat = 20
    static let xxl: CGFloat = 24; static let xxxl: CGFloat = 32
}

enum HitTarget {
    static let minimum: CGFloat = 24       // Desktop
    static let comfortable: CGFloat = 32
    static let touch: CGFloat = 44         // Mobile
}

enum Typography {
    static let serifFamily = "Cormorant Garamond"  // Headlines
    static let sansFamily = "Lato"                  // Body, UI
    static let monoFamily = "JetBrains Mono"        // Code

    static func heroTitle(_ size: CGFloat = 32) -> Font
    static func sectionTitle(_ size: CGFloat = 20) -> Font
    static func body(_ size: CGFloat = 14) -> Font
    static func caption(_ size: CGFloat = 12) -> Font
    static func code(_ size: CGFloat = 13) -> Font
}

// Views/BrandColors.swift
struct LoopflowPalette {
    let background, surface, surfaceMuted, border: Color
    let text, textSecondary: Color
    let accent: Color           // Burgundy #722F37
    let accentHover: Color
    static let light: LoopflowPalette
    static let dark: LoopflowPalette
}

// Status colors (in LoopflowCore)
extension Color {
    static let statusSuccess = Color(hex: 0x2D6A4F)   // Green
    static let statusError = Color(hex: 0xB45309)     // Orange
    static let statusWarning = Color(hex: 0xB0812A)   // Yellow-brown
    static let statusInfo = Color(hex: 0x0AB3CC)      // Cyan
    static let statusNeutral = Color(hex: 0x8B8B8B)   // Gray
}

// Button styles
struct DarkButtonStyle: ButtonStyle        // Primary actions
struct GhostButtonStyle: ButtonStyle       // Secondary actions
struct DestructiveButtonStyle: ButtonStyle // Delete, Stop
```

### Bundled Fonts

Registered in `ConcertoApp.swift`:
- Cormorant Garamond (Regular, Medium, SemiBold, Bold, Italic)
- Lato (Regular, Bold, Italic, BoldItalic)
- JetBrains Mono (Regular)

---

## Concerto — Automation

```swift
// ScriptCommands.swift — AppleScript handler
class CaptureScreenshotCommand: NSScriptCommand {
    override func performDefaultImplementation() -> Any?
    // Captures key window via SnapshotService, returns path
}
// Defined in Concerto.sdef, invoked by scripts/generate_screenshots.py

// Flags.swift
enum Flags {
    static var beta: Bool     // UserDefaults "beta"
    static func setBeta(_ enabled: Bool)
}
```

---

## Key Patterns

**State management**: `@Observable` (Swift 5.9+) with `@Environment` injection. No Combine (except GhosttyManager which uses `@Published`).

**Data flow**:
```
User Action → RepoState → LocalWaveService (HTTP to lfd:2486)
                  ↓
           WaveStore (dictionary, optimistic mutations)
                  ↓
        @Observable → SwiftUI views

WebSocket: LocalEventService → RepoState.handleEvent → WaveStore → views
Output:    streamOutput() → OutputBuffer → live output views
```

**Optimistic updates**: `WaveStore.applyOptimistic` snapshots state, applies mutation immediately for UI responsiveness, then `commitMutation` or `rollback` after API response.

**Wave lifecycle**: Create → Configure (flow, area, direction) → Run → [Running → Waiting → Running...] → Land (merge PR) or Stop.

**Concurrency**: Swift 6 strict concurrency. `@MainActor` on all state/UI. `Sendable` on all models. `actor` for `LocalEventService`.

**Conditional compilation**: `#if canImport(GhosttyKit)` / `GHOSTTY_ENABLED` for embedded terminal.

---

## Tests

```
ConcertoTests/
├── AuthServiceTests.swift
├── GhosttyTests.swift
├── RunStoreTests.swift
├── WaveRowTests.swift
└── WaveStoreTests.swift

ConcertoUITests/
└── ScreenshotPipelineTests.swift  # UI test modes: empty/sample/mock-waves

SymphoniaTests/
└── SymphoniaTests.swift           # Placeholder
```

Run: `swift test --package-path swift`

---

## Symphonia (placeholder)

```swift
// Symphonia/SymphoniaApp.swift — minimal app shell
// Symphonia/Views/PlaceholderView.swift — "Coming soon" message
```
