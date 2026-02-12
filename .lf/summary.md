# Swift Codebase Summary

Package `LoopflowSwift` — macOS 15+, Swift 6.0. Two apps + shared library.

```
swift/
├── Package.swift
├── LoopflowCore/           # Shared library (models + services)
│   ├── LoopflowCore.swift  # Config constants
│   ├── Models/
│   │   ├── AppPreferences.swift
│   │   ├── AppearanceMode.swift
│   │   ├── Flow.swift
│   │   ├── PullRequest.swift
│   │   ├── StatusColors.swift
│   │   ├── Step.swift
│   │   ├── Wave.swift
│   │   ├── WaveRun.swift
│   │   └── WaveViewModel.swift
│   └── Services/
│       ├── AuthError.swift
│       ├── AuthService.swift
│       ├── AuthState.swift
│       ├── LocalEventService.swift
│       ├── LocalWaveService.swift
│       ├── LoggingService.swift
│       ├── NotificationService.swift
│       ├── TokenProvider.swift
│       └── WaveServiceProtocol.swift
├── Concerto/               # macOS app (individual devs)
│   ├── ConcertoApp.swift
│   ├── BrandColors.swift
│   ├── DesignSystem.swift
│   ├── Flags.swift
│   ├── ScriptCommands.swift
│   ├── Models/RecentRepo.swift
│   ├── Services/
│   │   ├── AppIconProvider.swift
│   │   ├── Ghostty/GhosttyManager.swift
│   │   ├── Ghostty/GhosttyTerminalView.swift
│   │   ├── Ghostty/GhosttyTypes.swift
│   │   ├── NameGenerator.swift
│   │   ├── RecentAreasService.swift
│   │   ├── RecentsService.swift
│   │   ├── SetupService.swift
│   │   ├── SnapshotService.swift
│   │   └── TerminalLauncher.swift
│   ├── State/
│   │   ├── OutputBuffer.swift
│   │   ├── RepoState.swift
│   │   ├── RunStore.swift
│   │   └── WaveStore.swift
│   └── Views/
│       ├── AreaTypeahead.swift
│       ├── CommandPalette.swift
│       ├── ContentView.swift
│       ├── DiagnosticsView.swift
│       ├── DirectionTypeahead.swift
│       ├── EmbeddedTerminalPanel.swift
│       ├── FlowProgressPills.swift
│       ├── FlowTypeahead.swift
│       ├── InteractiveSessionView.swift
│       ├── IterationTimeline.swift
│       ├── LiveOutput.swift
│       ├── NextActionsBar.swift
│       ├── QuickExperimentView.swift
│       ├── RepoWindow.swift
│       ├── ScreenshotWindow.swift
│       ├── SetupView.swift
│       ├── StepRunner.swift
│       ├── TerminalTestWindow.swift
│       ├── TypeaheadComponents.swift
│       ├── WaitingStateCard.swift
│       ├── WaveDetailPanel.swift
│       ├── WaveRow.swift
│       ├── WaveRunsTab.swift
│       ├── WaveSidebar.swift
│       └── WelcomeWindow.swift
├── Symphonia/              # macOS app (teams, placeholder)
├── ConcertoTests/
└── SymphoniaTests/
```

## Dependencies

```swift
// Package.swift
.package(url: "https://github.com/nalexn/ViewInspector.git", from: "0.10.0")
.binaryTarget(name: "GhosttyKit",
    url: "https://bin.loopflow.studio/GhosttyKit-061a0ae.xcframework.zip", ...)
```

Concerto links: Carbon, QuartzCore, Metal, IOKit, libc++. Defines `GHOSTTY_ENABLED`.

## Build & Test

```bash
./dev run           # Build and launch
./dev run-debug     # Build with stdout visible
./dev test          # Run tests
./dev ui-test       # Generate Xcode project, run UI tests
./dev xcode         # Open in Xcode
./dev release       # Build release .app and .dmg
swift test --package-path swift  # SPM tests
```

---

## LoopflowCore — Models

### Configuration

```swift
// LoopflowCore.swift
public let lfdDefaultPort = 2486
public let lfdBaseURL = URL(string: "http://127.0.0.1:2486")!
public let lfdApiBaseURL = lfdBaseURL.appendingPathComponent("v0")
```

### AppPreferences

```swift
public enum TerminalApp: String, Sendable, CaseIterable {
    case warp, iterm, terminal, kitty
    public var displayName: String  // "Warp", "iTerm", "Terminal", "Kitty"
}

public enum IDEApp: String, Sendable, CaseIterable {
    case cursor, vscode, zed
    public var displayName: String  // "Cursor", "VS Code", "Zed"
}
```

### AppearanceMode

```swift
public enum AppearanceMode: String, Sendable, CaseIterable {
    case system, light, dark
    public var menuTitle: String
    public var colorScheme: ColorScheme?  // nil for system
}
```

### Flow

```swift
public enum FlowType: String, Sendable, Codable {
    case flow, step
}

/// A flow definition with name and steps.
public struct Flow: Sendable, Codable, Identifiable, Equatable {
    public var id: UUID = UUID()
    public var name: String
    public var steps: [Step]
    public var type: FlowType

    // Decodes steps as strings or objects: ["design"] or [{prompt: "design", config: ...}]
}
```

### Step

```swift
/// Per-step configuration overrides.
public struct StepConfig: Sendable, Codable, Equatable {
    public var model: String?
    public var direction: String?
    public var context: [String]?
    public var isEmpty: Bool
}

/// A Step is the basic unit — a prompt to run with optional config.
public struct Step: Sendable, Codable, Equatable, Identifiable {
    public var id: UUID = UUID()
    public var prompt: String
    public var config: StepConfig?
    // Handles string shorthand: "design" -> Step(prompt: "design")
}

/// StepRun represents a single execution of a step.
/// Schema matches Python's lfd/models.py StepRun class.
public struct StepRun: Sendable, Identifiable, Codable, Hashable {
    public let id: String
    public let step: String       // step name (was "prompt")
    public let repo: String
    public let worktree: String
    public let status: String     // "running", "waiting", "completed", "error"
    public let startedAt: Date    // JSON key: "started_at"
    public let endedAt: Date?     // JSON key: "ended_at"
    public let model: String
    public let runMode: String    // JSON key: "run_mode"

    public var isRunning: Bool    // status == "running" || "waiting"
    public var isCompleted: Bool  // status == "completed"
    public var isError: Bool      // status == "error"
}
```

### PullRequest

```swift
public enum PRState: String, Sendable, Codable {
    case open, merged, closed, draft
    public var displayText: String
}

public struct PullRequest: Sendable, Hashable, Codable {
    public let url: URL
    public let number: Int?
    public let state: PRState?
    public let title: String?
    public let branch: String?
}
```

### Wave

```swift
/// Determines when a wave runs. Stored in the stimuli table.
public struct Stimulus: Sendable, Hashable, Codable, Identifiable {
    public enum Kind: String, Sendable, Codable, CaseIterable {
        case loop, watch, cron
        public var icon: String    // "repeat", "eye", "clock"
        public var label: String   // "Loop", "Watch", "Schedule"
    }
    public var id: String
    public var kind: Kind
    public var cron: String?
    public var description: String  // "loop", "watch", or "cron(expr)"
}

public enum WaveStatus: String, Sendable, Codable {
    case idle, running, waiting, failed, paused
    public var color: Color   // statusSuccess/Warning/Neutral/Error
    public var icon: String   // SF Symbols: circle.fill, xmark.circle.fill, etc.
}

public enum MergeMode: String, Sendable, Codable {
    case pr, land
}

public enum WaitingReason: Sendable, Hashable {
    case prLimitReached(open: Int, limit: Int)
    public var description: String  // "3/5 PRs open"
}

/// An interactive session running in the embedded terminal.
public struct InteractiveSession: Sendable, Identifiable {
    public let id: String
    public let waveId: String
    public let step: String
    public let worktreePath: String
    public let prompt: String?
    public let startedAt: Date
    public var command: String  // "lf <step> '<prompt>'"
}

public struct CommitEntry: Sendable, Hashable, Identifiable {
    public let sha: String
    public let message: String
    public var id: String { sha }
}

/// Wave — an autonomous AI coding wave.
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
```

### WaveRun

```swift
public enum WaveRunStatus: String, Sendable, Codable {
    case pending, running, waiting, completed, failed, cancelled
    public var color: Color
}

/// A single execution of a Wave.
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

    public var duration: String?      // "3m42s"
    public var relativeTime: String   // "5 min. ago" (from endedAt ?? startedAt ?? createdAt)
}
```

### WaveViewModel

```swift
/// Rich view model wrapping Wave with git state, PR info, and display helpers.
public struct WaveViewModel: Sendable, Identifiable, Hashable {
    public var api: Wave
    public var worktreePath: String?     // from activeRun?.worktree ?? api.localWorktree
    public var branch: String?           // from activeRun?.branch ?? api.remoteBranch
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
    public var prLimit: Int              // default: 5
    public var mergeMode: MergeMode      // default: .pr
    public var pid: Int?
    public var lastMainSha: String?
    public var waitingReason: WaitingReason?
    public var runStartedAt: Date?

    // Proxied from api:
    public var id: String { api.id }
    public var name: String { get/set -> api.name }
    public var repo: String { get/set -> api.repo }
    public var flow: String { get/set -> api.flow }
    public var direction: [String] { get/set -> api.direction }
    public var area: [String] { get/set -> api.area }
    public var stimuli: [Stimulus] { get/set -> api.stimuli }
    public var stimulus: Stimulus? { stimuli.first }
    public var status: WaveStatus { get/set -> api.status }
    public var iteration: Int { get/set -> api.iteration }
    public var activeRun: WaveRun? { get/set -> api.activeRun }
    public var createdAt: Date? { get/set -> api.createdAt }
    public var stepIndex: Int { activeRun?.stepIndex ?? 0 }
    public var commits: [CommitEntry] { api.commits }
    public var diffStat: String? { api.diffStat }
    public var flowSteps: [String] { api.flowSteps }
    public var openPRCount: Int { api.openPRCount }

    // Computed display properties:
    public var shortId: String           // first 7 chars of id
    public var displayName: String       // name or "area · flow"
    public var areaDisplay: String       // joined areas
    public var directionDisplay: String  // joined directions
    public var statusText: String        // "Running", "Idle", etc.
    public var iterationText: String     // "iter 3" or ""
    public var detailText: String        // "area · flow · stimulus"
    public var stimulusText: String      // stimulus description or "manual"
    public var hasActiveStimulus: Bool
    public var statusIndicator: (icon: String, color: Color)
    public var effectiveOpenPRCount: Int
    public var hasOpenPRs: Bool
    public var pendingPR: (number: Int, url: URL?)?
    public var lastActivityAt: Date?
    public var lastActivityDescription: String?  // "implement 5 min. ago"
}
```

### StatusColors

```swift
extension Color {
    public static let statusSuccess = Color(hex: 0x2D6A4F)  // green
    public static let statusError   = Color(hex: 0xB45309)  // orange-red
    public static let statusWarning = Color(hex: 0xB0812A)  // yellow-orange
    public static let statusInfo    = Color(hex: 0x0AB3CC)  // cyan
    public static let statusNeutral = Color(hex: 0x8B8B8B)  // gray
}
```

---

## LoopflowCore — Services

### WaveServiceProtocol

```swift
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

### LocalWaveService

HTTP client for lfd daemon. Implements `WaveServiceProtocol`.

```swift
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

public struct CollapsePRsResult: Sendable {
    public let newPRUrl: String?
    public let closedPRs: [Int]
}

public struct AbsorbIntoPRResult: Sendable {
    public let targetBranch: String
    public let commitsAbsorbed: Int
}

public enum WaveServiceError: LocalizedError {
    case commandFailed(String)
}

public struct LocalWaveService: WaveServiceProtocol, @unchecked Sendable {
    // Two URLSession configs:
    //   session: 3s request / 10s resource timeout (fast fail)
    //   longSession: 30s request / 60s resource (git operations)

    // Wave CRUD
    func listWaves(repo: URL) async throws -> [Wave]        // GET /v0/waves?repo=&expand[]=active_run
    func getWave(_ id: String) async throws -> Wave          // GET /v0/waves/:id?expand[]=active_run
    func createWave(name:, repo:) async throws -> Wave       // POST /v0/waves {repo, name, flow:"design"}
    func updateWave(_ id:, config:) async throws -> Wave     // PATCH /v0/waves/:id
    func deleteWave(_ id:) async throws                      // DELETE /v0/waves/:id
    func cloneWave(_ id:, name:) async throws -> Wave        // POST /v0/waves/:id/clone

    // Actions
    func run(_ id:, overrides:) async throws                 // POST /v0/waves/:id/run
    func stop(_ id:) async throws                            // POST /v0/waves/:id/stop
    func landWave(_ id:) async throws                        // POST /v0/waves/:id/land {create_pr: true}
    func nextWave(_ id:) async throws -> String              // POST /v0/waves/:id/next -> new_branch
    func connect(_ id:) async throws -> ConnectionInfo       // POST /v0/waves/:id/connect

    // Stimulus
    func addStimulus(_ waveId:, kind:, cron:) async throws -> Stimulus     // POST /v0/waves/:id/stimulus
    func removeStimulus(_ waveId:, stimulusId:) async throws               // DELETE /v0/waves/:id/stimulus/:sid

    // Runs
    func listWaveRuns(waveId:, repo:, limit: 50) async throws -> [WaveRun]  // GET /v0/wave_runs

    // PR operations
    func collapsePRs(_ id:) async throws -> CollapsePRsResult    // POST /v0/waves/:id/collapse
    func absorbIntoPR(_ id:, prNumber:) async throws -> AbsorbIntoPRResult  // POST /v0/waves/:id/absorb

    // Output streaming
    func streamOutput(waveId:) -> AsyncThrowingStream<String, Error>  // GET /v0/waves/:id/logs (SSE)

    // Flows
    func listFlowsAndDirections(repo:) async throws -> WaveFlowsResult  // GET /v0/flows?repo=

    // Setup
    func connectLfd() async throws       // runs `lfd install`
    func checkAvailability() async -> Bool  // GET /status

    // JSON parsing (manual, not Codable)
    static func parseWaveFromJSON(_ json: [String: Any]) -> Wave
}
```

### LocalEventService

WebSocket subscription to `ws://127.0.0.1:2486/ws`. Auto-reconnects with exponential backoff (1s for first 10 attempts, then 5s).

```swift
public struct ConnectedEvent: Sendable {
    public let timestamp: Date
    public let waves: [Wave]
}

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

public struct WorktreeEvent: Sendable {
    public let worktree: String
    public let repo: String
    public let branch: String?
    public let timestamp: Date
}

public struct AgentStartedEvent: Sendable {
    public let agentId: String
    public let step: String
    public let worktree: String
    public let timestamp: Date
}

public struct AgentEndedEvent: Sendable {
    public let agentId: String
    public let status: String
    public let timestamp: Date
}

public struct OutputEvent: Sendable {
    public let waveId: String
    public let agentId: String
    public let text: String
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
    public var isConnected: Bool
    public func subscribe(
        onEvent: @escaping @Sendable (LFDEvent) -> Void,
        onConnectionChange: @escaping @Sendable (Bool) -> Void
    ) async
    public func disconnect() async
}
```

### AuthService / AuthState

```swift
public enum AuthError: Error, Sendable, LocalizedError {
    case noCallback, invalidCallback, notAuthenticated
    case tokenExpired, sessionFailed, refreshFailed(String)
    case keychainWrite(OSStatus), keychainDelete(OSStatus)
    case unknown(Error)
}

/// Sign in via loopflow.studio (GitHub, Google, Apple). JWT stored in Keychain.
public final class AuthService: NSObject, @unchecked Sendable {
    // Keychain: service "studio.loopflow.auth", account "jwt"
    @MainActor public func signIn() async throws -> String
    public func signOut() throws
    public func currentToken() -> String?
    public func tokenExpiresAt() -> Date?
    public func refreshToken() async throws -> String
}

@MainActor @Observable
public final class AuthState {
    public private(set) var token: String?
    public private(set) var isLoading: Bool
    public private(set) var error: AuthError?
    public var isAuthenticated: Bool    // token != nil && !isExpired
    public var isExpired: Bool
    public var needsRefresh: Bool       // within 24h of expiry

    public func signIn() async
    public func signOut()
    // Auto-refreshes token every hour
}

public protocol TokenProvider: Sendable {
    func token() async throws -> String
}
public struct NoAuthProvider: TokenProvider { ... }
public final class KeychainTokenProvider: TokenProvider { ... }
```

### LoggingService

```swift
/// Logs to ~/Library/Logs/Concerto/<category>.log
public enum LoggingService {
    public enum Category: String {
        case worktrees, lfd, general, ui, model
    }
    public static func append(_ message: String, category: Category = .worktrees)
    public static func ui(_ message: String)
    public static func model(_ message: String)
    public static func lfd(_ message: String)
    public static func read(category: Category = .worktrees) -> String
    public static func logPath(category: Category = .worktrees) -> String
    public static func logDirectory() -> URL
}
```

### NotificationService

```swift
public final class NotificationService: NSObject, UNUserNotificationCenterDelegate {
    public static let shared = NotificationService()
    public func requestAuthorization() async throws
    public func notifyNeedsInteractive(waveId: String, waveName: String, step: String)
    public func notifyError(waveId: String, waveName: String, message: String)
    public func notifyPRReady(waveId: String, waveName: String, prNumber: Int)
}
```

---

## Concerto — App

### ConcertoApp

```swift
@main
struct ConcertoApp: App {
    @State private var recentsService = RecentsService()
    @AppStorage("appearanceMode") private var appearanceMode = AppearanceMode.system.rawValue

    var body: some Scene {
        WindowGroup { /* Welcome or Screenshot or UITest mode */ }
        WindowGroup(id: "repo", for: URL.self) { /* RepoWindow */ }
        Window("Terminal Test", id: "terminal-test") { /* GhosttyTerminalView */ }
        .commands {
            // Beta toggle, Cmd+K command palette, Cmd+4 snapshot, appearance picker
        }
    }
}
```

### Flags

```swift
enum Flags {
    static var beta: Bool
    static func setBeta(_ enabled: Bool)
}
```

### BrandColors

```swift
extension Color {
    static let loopflowBurgundy = Color(hex: 0x722F37)
    static let loopflowBurgundyHover = Color(hex: 0x8B3D47)
    static let loopflowCream = Color(hex: 0xFAF8F5)
    static let loopflowCreamElevated = Color(hex: 0xFFFDFB)
    static let loopflowCreamMuted = Color(hex: 0xF3EEE7)
    static let loopflowSlate = Color(hex: 0x2B3036)
    static let loopflowSlateElevated = Color(hex: 0x343B44)
    static let loopflowSlateMuted = Color(hex: 0x3C4550)
    static let loopflowText = Color(hex: 0x1A1A1A)
    static let loopflowTextSecondary = Color(hex: 0x6B6B6B)
    static let loopflowTextLight = Color(hex: 0xF5F1EA)
    static let loopflowTextSecondaryLight = Color(hex: 0xC8C1B8)
    static let loopflowInfo = Color(hex: 0x0AB3CC)
}

struct LoopflowPalette {
    let background, surface, surfaceMuted, border: Color
    let text, textSecondary, accent, accentHover: Color
    static func make(for scheme: ColorScheme) -> LoopflowPalette
    // Light: cream bg, dark text, burgundy accent
    // Dark: slate bg, light text, burgundy accent
}
```

### DesignSystem

```swift
enum Spacing {
    static let xxs: CGFloat = 2;  static let xs: CGFloat = 4
    static let sm: CGFloat = 8;   static let md: CGFloat = 12
    static let lg: CGFloat = 16;  static let xl: CGFloat = 20
    static let xxl: CGFloat = 24; static let xxxl: CGFloat = 32
}

enum HitTarget {
    static let minimum: CGFloat = 24      // Desktop
    static let comfortable: CGFloat = 32
    static let touch: CGFloat = 44        // Mobile
}

enum ZIndex {
    static let base: Double = 0;     static let dropdown: Double = 100
    static let modal: Double = 200;  static let toast: Double = 300
    static let tooltip: Double = 400
}

enum CornerRadius {
    static let sm: CGFloat = 4;  static let md: CGFloat = 8
    static let lg: CGFloat = 12; static let xl: CGFloat = 16
    static let full: CGFloat = 9999
}

enum Typography {
    static let serifFamily = "Cormorant Garamond"  // Headlines
    static let sansFamily = "Lato"                  // Body, UI
    static let monoFamily = "JetBrains Mono"        // Code

    static func heroTitle(_ size: CGFloat = 32) -> Font
    static func sectionTitle(_ size: CGFloat = 20) -> Font
    static func body(_ size: CGFloat = 14) -> Font
    static func bodyBold(_ size: CGFloat = 14) -> Font
    static func caption(_ size: CGFloat = 12) -> Font
    static func code(_ size: CGFloat = 13) -> Font
    static func codeSmall(_ size: CGFloat = 11) -> Font
}

enum DesignAnimation {
    static func standard(_ reduceMotion: Bool) -> Animation?  // 0.2s easeInOut
    static func fast(_ reduceMotion: Bool) -> Animation?      // 0.1s easeOut
    static func spring(_ reduceMotion: Bool) -> Animation?    // spring response
}

extension View {
    func accessibleButton(_ label: String, hint: String? = nil) -> some View
    func accessibleToggle(_ label: String, isOn: Bool) -> some View
    func minHitTarget() -> some View    // 24×24pt minimum
    func keyboardFocusRing(_ isFocused: Bool, cornerRadius: CGFloat = CornerRadius.md) -> some View
}

struct DarkButtonStyle: ButtonStyle { ... }
```

---

## Concerto — State

### WaveStore

```swift
struct WaveGroups {
    let blocked: [WaveViewModel]         // status == .failed
    let pr: [WaveViewModel]              // hasOpenPRs
    let recentActivity: [WaveViewModel]  // activity within last hour, max 5
    let active: [WaveViewModel]          // running/waiting, not in recent
    let idle: [WaveViewModel]            // idle, not in recent
    var attentionCount: Int { blocked.count + pr.count }
    var openPRCount: Int
    var allInOrder: [WaveViewModel]
    static let empty: WaveGroups
}

@MainActor @Observable
final class WaveStore {
    private(set) var waves: [String: WaveViewModel]   // keyed by ID
    private(set) var ordered: [WaveViewModel]         // sorted: blocked, pr, recent, active, idle
    private(set) var groups: WaveGroups

    var onStatusChange: ((WaveViewModel, WaveStatus?, WaveStatus) -> Void)?

    func set(_ wave: WaveViewModel)
    func setAll(_ newWaves: [WaveViewModel])
    func remove(_ id: String) -> WaveViewModel?
    func removeAll()
    func wave(for id: String) -> WaveViewModel?

    // Optimistic mutations (prevent WebSocket events from overwriting during API calls)
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

### RunStore

```swift
@MainActor @Observable
final class RunStore {
    private(set) var runs: [String: [WaveRun]]  // keyed by wave ID, max 50 per wave

    func setRuns(for waveId: String, _ newRuns: [WaveRun])
    func runs(for waveId: String) -> [WaveRun]
    func clear(for waveId: String)
}
```

### OutputBuffer

```swift
struct OutputLine: Identifiable {
    let id: UUID
    let text: String
    let timestamp: Date
}

@MainActor @Observable
final class OutputBuffer {
    var interactiveSession: InteractiveSession?

    func appendOutput(waveId: String, text: String, timestamp: Date)
    func output(for waveId: String) -> [OutputLine]  // max 2000 lines per wave
    func clearOutput(for waveId: String)
    func startStreaming(waveId: String)    // HTTP SSE endpoint
    func stopStreaming(waveId: String)
    func recentOutput(for waveId: String, maxLength: Int = 60) -> String?

    func launchInteractiveSession(waveId:, step:, worktreePath:, prompt:)
    func endInteractiveSession()
    func hasActiveSession(for waveId: String) -> Bool
}
```

### RepoState

Central state manager. Composes WaveStore, RunStore, LocalWaveService, LocalEventService.

```swift
@MainActor @Observable
final class RepoState {
    var currentRepo: URL?
    var flows: [Flow]
    var availableDirections: [String]
    let waveStore: WaveStore
    let runStore: RunStore
    var selectedWaveId: String?
    var selectedWave: WaveViewModel?  // derived from waveStore
    var isLoading: Bool
    var errorMessage: String?
    var lfdConnected: Bool

    // Repo lifecycle
    func openRepo(_ url: URL, outputBuffer: OutputBuffer, skipBackgroundRefresh: Bool = false) async
    func startEventSubscription(outputBuffer: OutputBuffer)
    func connectLfd(outputBuffer: OutputBuffer) async throws
    func refreshWaves() async
    func refreshFlowsAsync() async

    // Wave CRUD (all with optimistic UI)
    func createWave(name: String) async throws         // auto-generates name via NameGenerator
    func runWave(wave:, area:, direction:, flow:) async throws
    func stopWave(_ wave: WaveViewModel) async throws
    func updateWave(_ wave:, area:, direction:, flow:, status:) async throws
    func renameWave(_ wave:, to newName:) async throws
    func deleteWave(_ wave: WaveViewModel) async throws
    func cloneWave(_ wave: WaveViewModel) async throws -> WaveViewModel
    func landWave(_ wave: WaveViewModel) async throws
    func nextWave(_ wave: WaveViewModel) async throws

    // Stimulus
    func addStimulus(wave:, kind:, cron:) async throws
    func removeStimulus(wave:, stimulusId:) async throws

    // PR operations
    func collapsePRs(_ waveId: String) async throws -> CollapsePRsResult
    func absorbIntoPR(_ waveId: String, prNumber: Int) async throws -> AbsorbIntoPRResult

    // Run history
    func loadRuns(for waveId: String)

    // In-flight tracking
    func isActionInFlight(_ waveId: String) -> Bool

    // UI test / screenshot support
    enum UITestMode: String { case emptyWorkspaces, sampleWorkspaces, mockWaves }
    struct ScreenshotMode { outputPath, repoPath, windowSize, selectBranch, mockLoops, mockConfig, selectTab }
    func configureMockWaves()
    func configureMockWavesEmpty()
}
```

---

## Concerto — Services

### GhosttyManager

Embedded terminal via GhosttyKit xcframework. Custom Loopflow theme (burgundy background #4A1A2C, cream foreground #F5E6D3).

```swift
@MainActor
final class GhosttyManager: ObservableObject {
    enum State: Equatable {
        case uninitialized, initializing, ready, failed(String)
    }
    @Published private(set) var state: State
    var onSessionClosed: (() -> Void)?
    static let shared: GhosttyManager

    func initialize()
    func tick()
    func createSurface(workingDirectory: String, command: String?, view: NSView) -> ghostty_surface_t?
    func destroySurface(_ surface: ghostty_surface_t)
    func registerActiveSession(_ surface: ghostty_surface_t, sessionId: String)
    func destroyActiveSession()
    func sendText(_ text: String)
}
// Compiled out (stub) when GHOSTTY_ENABLED is not defined.
```

### TerminalLauncher

```swift
struct TerminalLauncher {
    func launchTerminal(_ terminal: TerminalApp, at path: URL, command: String?) throws
    func openInIDE(_ ide: IDEApp, at path: URL, workspace: String?) throws
    func openInFinder(at path: URL)
    func openURL(_ url: URL)
    func launchStep(_ step: String, terminal: TerminalApp, at repo: URL) throws
}
// Uses AppleScript for Warp/iTerm/Terminal. Process for Kitty/Cursor/VSCode/Zed.
```

### Other Services

```swift
// NameGenerator — generates "magical-musical" names for waves
enum NameGenerator {
    static func generate() -> String  // e.g. "aurora-allegro", "cascade-aria"
}

// RecentsService — recent repos, stored in UserDefaults (max 10)
@Observable final class RecentsService {
    private(set) var recentRepos: [RecentRepo]
    func addRecent(_ url: URL)
    func removeRecent(_ url: URL)
    func clearAll()
}

struct RecentRepo: Codable, Identifiable {
    let path: String
    let lastOpened: Date
    var url: URL
    var displayName: String
    var exists: Bool
}

// RecentAreasService — recent areas per repo, UserDefaults (max 5)
struct RecentAreasService {
    func recentAreas(for repoURL: URL) -> [String]
    func addRecentArea(_ area: String, for repoURL: URL)
}

// SetupService — dependency checks and daemon management
struct SetupService {
    struct DependencyStatus { var lfInstalled: Bool; var lfPath: String? }
    func checkDependencies() -> DependencyStatus
    func install() async throws
    func ensureDaemonRunning() async throws
}

// SnapshotService — screenshot capture (no Screen Recording permission needed)
@MainActor struct SnapshotService {
    func snapshotKeyWindow() throws -> URL     // -> /tmp/concerto-<timestamp>.png
    func snapshotWindow(_ window: NSWindow, to outputURL: URL) throws -> URL
}

// AppIconProvider — resolve app icons by bundle ID
enum AppIdentifier {
    case cursor, warp, vscode, iterm, terminal, zed, kitty, github
    var bundleIdentifier: String?
    var appName: String
}
struct AppIconProvider {
    static func icon(for app: AppIdentifier) -> NSImage?
    static func iconImage(for app: AppIdentifier, size: CGFloat = 24) -> Image
}
```

---

## Concerto — Key Views

```
RepoWindow                — per-repo window; holds RepoState + OutputBuffer
  └─ ContentView          — main layout (sidebar + detail)
      ├─ WaveSidebar      — grouped wave list (blocked, pr, recent, active, idle)
      │   └─ WaveRow      — wave row (status, name, area, flow, PR badge, live output)
      ├─ WaveDetailPanel   — selected wave config + actions
      │   ├─ FlowTypeahead, AreaTypeahead, DirectionTypeahead — autocomplete inputs
      │   ├─ FlowProgressPills — step progress indicators
      │   ├─ IterationTimeline — visual timeline of run iterations
      │   ├─ NextActionsBar — Land / Next / Collapse / Absorb buttons
      │   ├─ WaitingStateCard — PR limit reached state
      │   ├─ WaveRunsTab   — historical runs with PR state
      │   └─ LiveOutput    — scrolling agent output
      ├─ EmbeddedTerminalPanel — Ghostty terminal for interactive sessions
      │   └─ InteractiveSessionView — session controls
      └─ StepRunner        — run a single step on demand

ScreenshotWindow     — screenshot mode (--snapshot flag), loads mock data
WelcomeWindow        — initial launch, recent repos picker
QuickExperimentView  — quick one-off experiment launcher
SetupView            — dependency installation flow
DiagnosticsView      — lfd connection status
CommandPalette       — Cmd+K global search (fuzzy)
```

### WaveRow Details

Sidebar row showing: display name (click-to-edit when selected), flow badge or PR badge, area, iteration, last activity timestamp (italic serif), stimulus label, live output preview (80px) when selected. Context menu: Delete Wave.

---

## Communication Architecture

Two channels, intentionally different:

1. **HTTP** (`LocalWaveService`) — CRUD operations, actions, output streaming
   - Base: `http://127.0.0.1:2486/v0/`
   - Two timeout tiers: fast (3s) for reads, long (30s) for git operations
   - JSON parsed manually (not Codable) via `parseWaveFromJSON`

2. **WebSocket** (`LocalEventService`) — live UI updates
   - URL: `ws://127.0.0.1:2486/ws`
   - Events: connected, wave_*, worktree_updated, agent_started/ended, output_line
   - Auto-reconnect with backoff (1s × 10, then 5s)

### Optimistic UI Pattern

All wave mutations follow: apply optimistic → API call → commit on success / rollback on failure.

```swift
func optimistic(_ id: String,
    mutation: (inout WaveViewModel) -> Void,
    apiCall: () async throws -> Void) async throws
```

Actions (run/stop/next) also schedule a safety-net refresh 10s after commit, since real state arrives via WebSocket.

---

## Tests

- `ConcertoTests/WaveStoreTests.swift` — optimistic mutations, rollback, pending state, grouping
- `ConcertoTests/WaveTests.swift` — Wave model parsing and behavior
- `ConcertoTests/RunStoreTests.swift` — caching, clearing, per-wave isolation
- `ConcertoTests/AuthServiceTests.swift` — auth flow
- `ConcertoTests/GhosttyTests.swift` — terminal embedding
- `ConcertoTests/WaveRowTests.swift` — view testing with ViewInspector

---

## Symphonia (Teams Product)

Placeholder. Just displays a basic view:

```swift
@main struct SymphoniaApp: App {
    var body: some Scene { WindowGroup { PlaceholderView() } }
}
```
