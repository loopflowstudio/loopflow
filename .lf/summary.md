# Swift Codebase Summary

Package: `LoopflowSwift` — macOS 15+, Swift 6
Products: `LoopflowCore` (library), `Concerto` (app), `Symphonia` (placeholder)
Dependencies: ViewInspector 0.10+, GhosttyKit (binary xcframework)

```
swift/
  Package.swift
  LoopflowCore/          # Shared models and services
    LoopflowCore.swift
    Models/
    Services/
  Concerto/              # macOS app — wave management UI
    ConcertoApp.swift
    BrandColors.swift
    DesignSystem.swift
    Flags.swift
    ScriptCommands.swift
    Models/
    Services/
    State/
    Views/               # 30+ SwiftUI views
  Symphonia/             # Teams product (placeholder)
  ConcertoTests/
  ConcertoUITests/
  SymphoniaTests/
```

---

## LoopflowCore

### Configuration

```swift
// swift/LoopflowCore/LoopflowCore.swift
public let lfdDefaultPort = 2486
public let lfdBaseURL = URL(string: "http://127.0.0.1:2486")!
public let lfdApiBaseURL = lfdBaseURL.appendingPathComponent("v0")
```

### Models

#### Wave — autonomous AI coding wave

```swift
// swift/LoopflowCore/Models/Wave.swift
public struct Stimulus: Sendable, Hashable, Codable, Identifiable {
    public enum Kind: String, Codable, CaseIterable { case loop, watch, cron }
    public var id: String
    public var kind: Kind
    public var cron: String?           // Only for .cron
}

public enum WaveStatus: String, Codable { case idle, running, waiting, failed, paused }
public enum MergeMode: String, Codable { case pr, land }

public enum WaitingReason: Sendable, Hashable {
    case prLimitReached(open: Int, limit: Int)
}

public struct InteractiveSession: Sendable, Identifiable {
    public let id, waveId, step, worktreePath: String
    public let prompt: String?
    public let startedAt: Date
    public var command: String          // "lf <step> <prompt>"
}

public struct CommitEntry: Sendable, Hashable, Identifiable {
    public let sha: String
    public let message: String
}

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

#### WaveRun — single execution of a wave

```swift
// swift/LoopflowCore/Models/WaveRun.swift
public enum WaveRunStatus: String, Codable {
    case pending, running, waiting, completed, failed, cancelled
}

public struct WaveRun: Sendable, Identifiable, Hashable {
    public let id: String
    public let waveId: String?
    public let flow, area, repo: String
    public let direction: [String]
    public var status: WaveRunStatus
    public var iteration, stepIndex: Int
    public var worktree, branch, currentStep, error: String?
    public var pr: PullRequest?
    public var startedAt, endedAt: Date?
    public var createdAt: Date
    // Computed: duration -> "3m45s", relativeTime -> "2h ago"
}
```

#### WaveViewModel — enriched view model with git state

```swift
// swift/LoopflowCore/Models/WaveViewModel.swift
public struct WaveViewModel: Sendable, Identifiable, Hashable {
    public var api: Wave                  // Server-side wave data
    public var worktreePath, branch: String?
    public var isDirty, isRebasing, isMerging, hasDiff: Bool
    public var aheadMain, behindMain: Int
    public var aheadRemote, behindRemote: Int
    public var prURL: URL?
    public var prNumber: Int?
    public var prState: PRState?
    public var recentSteps: [StepRun]
    public var prLimit: Int               // Default 5
    public var mergeMode: MergeMode       // Default .pr
    public var pid: Int?
    public var lastMainSha: String?
    public var waitingReason: WaitingReason?
    public var runStartedAt: Date?

    // Delegated to api: id, name, repo, flow, direction, area, stimuli, status, iteration, activeRun, createdAt
    // Computed: shortId, displayName, areaDisplay, directionDisplay, statusText, iterationText, detailText
    // Computed: stimulusText, hasActiveStimulus, statusIndicator, pendingPR, lastActivityAt, hasOpenPRs
}
```

#### Flow / Step / StepRun

```swift
// swift/LoopflowCore/Models/Flow.swift
public enum FlowType: String, Codable { case flow, step }

public struct Flow: Sendable, Codable, Identifiable, Equatable {
    public var id: UUID
    public var name: String
    public var steps: [Step]
    public var type: FlowType
    // Decodes steps as strings or objects
}

// swift/LoopflowCore/Models/Step.swift
public struct StepConfig: Sendable, Codable, Equatable {
    public var model: String?
    public var direction: String?
    public var context: [String]?
}

public struct Step: Sendable, Codable, Equatable, Identifiable {
    public var id: UUID
    public var prompt: String
    public var config: StepConfig?
    // Decodes string shorthand: "design" -> Step(prompt: "design")
}

public struct StepRun: Sendable, Identifiable, Codable, Hashable {
    public let id, step, repo, worktree, status: String
    public let startedAt: Date
    public let endedAt: Date?
    public let model, runMode: String
    // CodingKeys: started_at, ended_at, run_mode
}
```

#### PullRequest

```swift
// swift/LoopflowCore/Models/PullRequest.swift
public enum PRState: String, Codable { case open, merged, closed, draft }

public struct PullRequest: Sendable, Hashable, Codable {
    public let url: URL
    public let number: Int?
    public let state: PRState?
    public let title, branch: String?
}
```

#### Preferences / Appearance

```swift
// swift/LoopflowCore/Models/AppPreferences.swift
public enum TerminalApp: String, CaseIterable { case warp, iterm, terminal, kitty }
public enum IDEApp: String, CaseIterable { case cursor, vscode, zed }

// swift/LoopflowCore/Models/AppearanceMode.swift
public enum AppearanceMode: String, CaseIterable { case system, light, dark }
```

#### Status Colors

```swift
// swift/LoopflowCore/Models/StatusColors.swift
extension Color {
    public static let statusSuccess = Color(hex: 0x2D6A4F)   // Green
    public static let statusError = Color(hex: 0xB45309)     // Amber
    public static let statusWarning = Color(hex: 0xB0812A)   // Gold
    public static let statusInfo = Color(hex: 0x0AB3CC)      // Teal
    public static let statusNeutral = Color(hex: 0x8B8B8B)   // Gray
}
```

### Services

#### WaveServiceProtocol — interface for wave operations

```swift
// swift/LoopflowCore/Services/WaveServiceProtocol.swift
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

#### LocalWaveService — HTTP client for lfd daemon

```swift
// swift/LoopflowCore/Services/LocalWaveService.swift
public struct WaveConfigUpdate: Sendable {
    public var name: String?, area: [String]?, direction: [String]?, flow: String?, status: WaveStatus?
}
public struct RunOverrides: Sendable {
    public var area: [String]?, direction: [String]?, flow: String?
}
public struct ConnectionInfo: Sendable {
    public let worktree, step, agentId, promptFile: String
    public let waveRunId: String?
    public let stepIndex: Int
}
public struct CollapsePRsResult: Sendable { public let newPRUrl: String?; public let closedPRs: [Int] }
public struct AbsorbIntoPRResult: Sendable { public let targetBranch: String; public let commitsAbsorbed: Int }
public enum WaveServiceError: LocalizedError { case commandFailed(String) }

public struct LocalWaveService: WaveServiceProtocol {
    // Base: http://127.0.0.1:2486/v0
    // Timeouts: 3s/10s (default), 30s/60s (long — git ops)
    //
    // API endpoints:
    //   GET    /v0/waves?repo=...&expand[]=active_run      -> listWaves
    //   GET    /v0/waves/:id?expand[]=active_run            -> getWave
    //   POST   /v0/waves                                    -> createWave
    //   PATCH  /v0/waves/:id                                -> updateWave
    //   DELETE /v0/waves/:id                                -> deleteWave
    //   POST   /v0/waves/:id/run                            -> run (with overrides)
    //   POST   /v0/waves/:id/stop                           -> stop
    //   POST   /v0/waves/:id/land                           -> landWave
    //   POST   /v0/waves/:id/next                           -> nextWave
    //   POST   /v0/waves/:id/clone                          -> cloneWave
    //   POST   /v0/waves/:id/connect                        -> connect (interactive)
    //   POST   /v0/waves/:id/stimulus                       -> addStimulus
    //   DELETE /v0/waves/:id/stimulus/:stimId                -> removeStimulus
    //   POST   /v0/waves/:id/collapse                       -> collapsePRs
    //   POST   /v0/waves/:id/absorb                         -> absorbIntoPR
    //   GET    /v0/waves/:id/logs                           -> streamOutput (SSE)
    //   GET    /v0/wave_runs?wave_id=...&repo=...&limit=    -> listWaveRuns
    //   GET    /v0/flows?repo=...                           -> listFlowsAndDirections
    //   GET    /status                                      -> checkAvailability
    //
    // JSON parsing: manual JSONSerialization, not Codable
    // Status normalization: "error" -> "failed", "completed" -> "idle"
    func streamOutput(waveId: String) -> AsyncThrowingStream<String, Error>
    func connectLfd() async throws          // runs "lfd install"
    func checkAvailability() async -> Bool
}
```

#### LocalEventService — WebSocket subscriber for real-time updates

```swift
// swift/LoopflowCore/Services/LocalEventService.swift
public struct ConnectedEvent: Sendable { let timestamp: Date; let waves: [Wave] }
public enum WaveEventType: String { case created, updated, deleted, started, stopped, waiting }
public struct WaveEvent: Sendable { let type: WaveEventType; let waveId: String; let wave: Wave?; ... }
public struct WorktreeEvent: Sendable { let worktree, repo: String; let branch: String? }
public struct AgentStartedEvent: Sendable { let agentId, step, worktree: String }
public struct AgentEndedEvent: Sendable { let agentId, status: String }
public struct OutputEvent: Sendable { let waveId, agentId, text: String }

public enum LFDEvent: Sendable {
    case connected(ConnectedEvent)
    case wave(WaveEvent)
    case worktree(WorktreeEvent)
    case agentStarted(AgentStartedEvent)
    case agentEnded(AgentEndedEvent)
    case output(OutputEvent)
}

public actor LocalEventService {
    // ws://127.0.0.1:2486/ws
    // Auto-reconnect: 1s for first 10 attempts, then 5s
    public var isConnected: Bool
    public func subscribe(onEvent: @Sendable (LFDEvent) -> Void, onConnectionChange: @Sendable (Bool) -> Void) async
    public func disconnect() async
}
```

#### Auth

```swift
// swift/LoopflowCore/Services/AuthError.swift
public enum AuthError: Error, Sendable {
    case noCallback, invalidCallback, notAuthenticated, tokenExpired, sessionFailed
    case refreshFailed(String), keychainWrite(OSStatus), keychainDelete(OSStatus), unknown(Error)
}

// swift/LoopflowCore/Services/AuthService.swift
public final class AuthService: NSObject, ASWebAuthenticationPresentationContextProviding {
    // OAuth via loopflow.studio, ASWebAuthenticationSession
    // Keychain: service="studio.loopflow.auth", account="jwt"
    public func signIn() async throws -> String     // Returns JWT
    public func signOut() throws
    public func currentToken() -> String?
    public func refreshToken() async throws -> String
    public func tokenExpiresAt() -> Date?
    static func decodeExpiry(_ token: String) -> Date?
}

// swift/LoopflowCore/Services/AuthState.swift
@MainActor @Observable
public final class AuthState {
    public private(set) var token: String?
    public private(set) var isLoading: Bool
    public private(set) var error: AuthError?
    public var isAuthenticated: Bool    // token != nil && !isExpired
    public var needsRefresh: Bool       // expires within 24h
    public func signIn() async
    public func signOut()
    // Auto-refresh monitor: checks hourly
}

// swift/LoopflowCore/Services/TokenProvider.swift
public protocol TokenProvider: Sendable { func token() async throws -> String }
public struct NoAuthProvider: TokenProvider { ... }
public final class KeychainTokenProvider: TokenProvider { ... }
```

#### Logging & Notifications

```swift
// swift/LoopflowCore/Services/LoggingService.swift
public enum LoggingService {
    public enum Category: String { case worktrees, lfd, general, ui, model }
    static func append(_ message: String, category: Category)
    static func ui(_ message: String)
    static func model(_ message: String)
    static func lfd(_ message: String)
    static func read(category:) -> String
    // Logs to ~/Library/Logs/Concerto/<category>.log
}

// swift/LoopflowCore/Services/NotificationService.swift
public final class NotificationService: UNUserNotificationCenterDelegate {
    public static let shared: NotificationService
    func requestAuthorization() async throws
    func notifyNeedsInteractive(waveId:, waveName:, step:)
    func notifyError(waveId:, waveName:, message:)
    func notifyPRReady(waveId:, waveName:, prNumber:)
    // Taps post Notification.Name.selectWave with waveId
}
```

---

## Concerto (macOS App)

### App Entry

```swift
// swift/Concerto/ConcertoApp.swift
@main struct ConcertoApp: App {
    // Scenes:
    //   WindowGroup: WelcomeWindow (default) | ScreenshotWindow | RepoWindow (UI test)
    //   WindowGroup(id: "repo", for: URL.self): RepoWindow — 900x700 default
    //   Window("Terminal Test", id: "terminal-test"): TerminalTestWindow
    // Commands: Beta toggle, Appearance picker, Snapshot (Cmd+4), Command Palette (Cmd+K)
    // Palette: resolved from AppearanceMode, injected via .environment(\.palette)
}

// swift/Concerto/Flags.swift
enum Flags { static var beta: Bool; static func setBeta(_ enabled: Bool) }
```

### Design System

```swift
// swift/Concerto/DesignSystem.swift
enum Spacing     { xxs=2, xs=4, sm=8, md=12, lg=16, xl=20, xxl=24, xxxl=32 }
enum HitTarget   { minimum=24, comfortable=32, touch=44 }
enum ZIndex      { base=0, dropdown=100, modal=200, toast=300, tooltip=400 }
enum CornerRadius { sm=4, md=8, lg=12, xl=16, full=9999 }

enum Typography {
    static let serifFamily = "Cormorant Garamond"   // Headlines
    static let sansFamily = "Lato"                   // Body/UI
    static let monoFamily = "JetBrains Mono"         // Code
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
    static func spring(_ reduceMotion: Bool) -> Animation?    // 0.3s spring
}

extension View {
    func accessibleButton(_ label:, hint:) -> some View
    func accessibleToggle(_ label:, isOn:) -> some View
    func minHitTarget() -> some View
    func keyboardFocusRing(_ isFocused:, cornerRadius:) -> some View
}

struct DarkButtonStyle: ButtonStyle { ... }  // Burgundy accent, cream text
```

### Brand Colors / Palette

```swift
// swift/Concerto/BrandColors.swift
extension Color {
    static let loopflowBurgundy = Color(hex: 0x722F37)
    static let loopflowBurgundyHover = Color(hex: 0x8B3D47)
    static let loopflowCream = Color(hex: 0xFAF8F5)
}

struct LoopflowPalette {
    let background, surface, surfaceMuted, border: Color
    let text, textSecondary: Color
    let accent, accentHover: Color

    static let light     // Cream backgrounds (#FAF8F5), dark text
    static let dark      // Slate backgrounds (#2B3036), light text
    static let deepWine  // Dark burgundy (#1E1215), accent #8B2252
}

// Environment injection
struct PaletteKey: EnvironmentKey { static let defaultValue = LoopflowPalette.light }
extension EnvironmentValues { var palette: LoopflowPalette { get/set } }
```

### State

#### RepoState — primary data state

```swift
// swift/Concerto/State/RepoState.swift
@MainActor @Observable final class RepoState {
    enum UITestMode: String { case emptyWorkspaces, sampleWorkspaces, mockWaves }
    struct ScreenshotMode { outputPath, repoPath?, windowSize?, selectBranch?, mockLoops, mockConfig?, selectTab? }

    var currentRepo: URL?
    var flows: [Flow]
    var availableDirections: [String]
    let waveStore: WaveStore
    let runStore: RunStore
    var waves: [WaveViewModel] { waveStore.ordered }
    var waveGroups: WaveGroups { waveStore.groups }
    var selectedWaveId: String?
    var selectedWave: WaveViewModel?     // Derived from selectedWaveId + waveStore
    var isLoading: Bool
    var errorMessage: String?
    var lfdConnected: Bool
    private(set) var inFlightActions: Set<String>

    // Lifecycle
    func openRepo(_ url: URL, outputBuffer: OutputBuffer) async
    func startEventSubscription(outputBuffer: OutputBuffer)
    func refreshWaves() async
    func refreshFlowsAsync() async
    func loadRuns(for waveId: String)
    func connectLfd(outputBuffer:) async throws

    // Wave CRUD — all use optimistic update pattern
    func createWave(name: String) async throws          // Pending insert -> API -> replace
    func runWave(wave:, area:, direction:, flow:) async throws
    func stopWave(_ wave:) async throws
    func landWave(_ wave:) async throws                 // In-flight action tracking
    func nextWave(_ wave:) async throws
    func updateWave(_ wave:, area:, direction:, flow:, status:) async throws
    func renameWave(_ wave:, to:) async throws
    func deleteWave(_ wave:) async throws
    func cloneWave(_ wave:) async throws -> WaveViewModel
    func addStimulus(wave:, kind:, cron:) async throws
    func removeStimulus(wave:, stimulusId:) async throws
    func collapsePRs(_ waveId:) async throws -> CollapsePRsResult
    func absorbIntoPR(_ waveId:, prNumber:) async throws -> AbsorbIntoPRResult

    // Mock data for UI tests
    func configureMockWaves()
    func configureForUITest(_ mode:, repoURL:)
}
```

**Optimistic pattern:** Snapshot state -> apply mutation -> API call -> commit on success / rollback on failure. Actions (run/stop/next) schedule safety-net refresh after 10s. Status changes fire macOS notifications.

#### WaveStore — dictionary-keyed wave state

```swift
// swift/Concerto/State/WaveStore.swift
struct WaveGroups {
    let blocked: [WaveViewModel]          // Failed
    let pr: [WaveViewModel]               // Has open PRs
    let recentActivity: [WaveViewModel]   // Activity in last hour (max 5)
    let active: [WaveViewModel]           // Running/waiting without recent activity
    let idle: [WaveViewModel]             // Idle without PRs
    var attentionCount: Int               // blocked + pr count
    var openPRCount: Int
    var allInOrder: [WaveViewModel]       // Concatenated in priority order
}

@MainActor @Observable final class WaveStore {
    private(set) var waves: [String: WaveViewModel]
    private(set) var ordered: [WaveViewModel]      // Recomputed on change
    private(set) var groups: WaveGroups             // Recomputed on change
    var onStatusChange: ((WaveViewModel, WaveStatus?, WaveStatus) -> Void)?

    func set(_ wave:)                    // Skipped if pending mutation
    func setAll(_ newWaves:)             // Preserves pending mutations
    func remove(_ id:) -> WaveViewModel?
    func removeAll()
    func applyOptimistic(_ id:, _ mutation:) -> WaveViewModel?  // Returns snapshot
    func commitMutation(_ id:)
    func rollback(_ snapshot:)
    func insertPending(_ wave:)
    func replacePending(_ pendingId:, with:)
    func removePending(_ id:)
    func applyDelete(_ id:)
    func wave(for id:) -> WaveViewModel?
}
```

#### RunStore / OutputBuffer

```swift
// swift/Concerto/State/RunStore.swift
@MainActor @Observable final class RunStore {
    private(set) var runs: [String: [WaveRun]]     // Max 50 per wave
    func setRuns(for waveId:, _ newRuns:)
    func runs(for waveId:) -> [WaveRun]
    func clear(for waveId:)
}

// swift/Concerto/State/OutputBuffer.swift
struct OutputLine: Identifiable { let id: UUID; let text: String; let timestamp: Date }

@MainActor @Observable final class OutputBuffer {
    var interactiveSession: InteractiveSession?     // One at a time
    // Max 2000 lines per wave
    func appendOutput(waveId:, text:, timestamp:)   // Skipped if stream active
    func output(for waveId:) -> [OutputLine]
    func clearOutput(for waveId:)
    func startStreaming(waveId:)                     // GET /v0/waves/:id/logs (replay + follow)
    func stopStreaming(waveId:)
    func recentOutput(for waveId:, maxLength:) -> String?
    func launchInteractiveSession(waveId:, step:, worktreePath:, prompt:)
    func endInteractiveSession()
    func hasActiveSession(for waveId:) -> Bool
}
```

### Services

```swift
// swift/Concerto/Services/SetupService.swift
struct SetupService {
    struct DependencyStatus { var lfInstalled: Bool; var lfPath: String? }
    func checkDependencies() -> DependencyStatus
    func install() async throws                     // Installs uv, loopflow, node, claude
    func ensureDaemonRunning() async throws         // Checks/loads com.loopflow.lfd LaunchAgent
    // Log: ~/.lf/logs/concerto-setup.log
    // Search dirs: ~/.local/bin, /opt/homebrew/bin, /usr/local/bin, /usr/bin, ~/.cargo/bin
}

// swift/Concerto/Services/TerminalLauncher.swift
struct TerminalLauncher {
    func launchTerminal(_ terminal: TerminalApp, at path: URL, command: String?) throws
    func openInIDE(_ ide: IDEApp, at path: URL, workspace: String?) throws
    func openInFinder(at path: URL)
    func openURL(_ url: URL)
    func launchStep(_ step: String, terminal: TerminalApp, at repo: URL) throws
    // Warp: AppleScript UI scripting (requires Accessibility)
    // iTerm: AppleScript "write text"
    // Terminal: AppleScript "do script"
    // Kitty: --single-instance process
    // Cursor/VSCode: finds .code-workspace; Zed: direct launch
}

// swift/Concerto/Services/RecentsService.swift
@Observable final class RecentsService {
    private(set) var recentRepos: [RecentRepo]      // Max 10, UserDefaults
    func addRecent(_ url: URL)
    func removeRecent(_ url: URL)
}

// swift/Concerto/Services/RecentAreasService.swift
struct RecentAreasService {
    func recentAreas(for repoURL: URL) -> [String]  // Max 5, UserDefaults
    func addRecentArea(_ area: String, for repoURL: URL)
}

// swift/Concerto/Services/NameGenerator.swift
enum NameGenerator {
    static let magical: [String]   // aurora, cascade, crystal, ...
    static let musical: [String]   // allegro, aria, ballad, ...
    static func generate() -> String   // "crystal-melody"
}

// swift/Concerto/Services/SnapshotService.swift
@MainActor struct SnapshotService {
    func snapshotKeyWindow() throws -> URL              // /tmp/concerto-<timestamp>.png
    func snapshotWindow(_ window: NSWindow, to outputURL: URL) throws -> URL
    // Uses bitmapImageRepForCachingDisplay — no Screen Recording permission needed
}

// swift/Concerto/Services/AppIconProvider.swift
enum AppIdentifier { case cursor, warp, vscode, iterm, terminal, zed, kitty, github }
struct AppIconProvider {
    static func icon(for app: AppIdentifier) -> NSImage?
    static func iconImage(for app: AppIdentifier, size: CGFloat) -> Image
}

// swift/Concerto/ScriptCommands.swift
class CaptureScreenshotCommand: NSScriptCommand  // AppleScript automation
```

#### Ghostty Integration — embedded terminal

```swift
// swift/Concerto/Services/Ghostty/GhosttyTypes.swift
enum TerminalStatus: Equatable { case initializing, running, completed(exitCode: Int32), failed(error: String) }
struct GhosttySession: Identifiable { let id, worktree: String; let command: [String]; var status: TerminalStatus }

// swift/Concerto/Services/Ghostty/GhosttyManager.swift
@MainActor final class GhosttyManager: ObservableObject {
    enum State: Equatable { case uninitialized, initializing, ready, failed(String) }
    @Published private(set) var state: State
    var onSessionClosed: (() -> Void)?
    static let shared: GhosttyManager

    func initialize()           // ghostty_init + config + app
    func createSurface(workingDirectory:, command:, view: NSView) -> ghostty_surface_t?
    func destroySurface(_ surface:)
    func registerActiveSession(_ surface:, sessionId:)
    func destroyActiveSession()
    func sendText(_ text:)
    func tick()                 // ghostty_app_tick via wakeup callback

    // Terminal theme: cream (#F5E6D3) on dark burgundy (#4A1A2C), font-size 13
}
// Stub implementation when GHOSTTY_ENABLED not defined

// swift/Concerto/Services/Ghostty/GhosttyTerminalView.swift
// GhosttyTerminalView: SwiftUI wrapper
// GhosttyMetalView: NSView + NSTextInputClient — keyboard, mouse, IME, DisplayLink rendering
```

### Models

```swift
// swift/Concerto/Models/RecentRepo.swift
struct RecentRepo: Codable, Identifiable {
    let path: String
    let lastOpened: Date
    var url: URL { URL(fileURLWithPath: path) }
    var displayName: String { URL(fileURLWithPath: path).lastPathComponent }
}
```

### Views

```
swift/Concerto/Views/
  WelcomeWindow.swift           — Recent repos list, open folder
  RepoWindow.swift              — Window wrapper with setup check
  ContentView.swift             — Split view: sidebar + detail panel
  WaveSidebar.swift             — Grouped wave list (blocked/pr/recent/active/idle)
  WaveRow.swift                 — Individual wave in sidebar
  WaveDetailPanel.swift         — Wave config, actions, output
  WaveRunsTab.swift             — Historical runs with PR state
  CommandPalette.swift          — Cmd+K fuzzy search (Linear-style)
  InteractiveSessionView.swift  — Embedded terminal for interactive steps
  EmbeddedTerminalPanel.swift   — Ghostty terminal wrapper
  LiveOutput.swift              — Streaming agent output display
  FlowProgressPills.swift       — Step progress visualization
  NextActionsBar.swift          — Land/Next buttons
  QuickExperimentView.swift     — No-wave quick launch
  StepRunner.swift              — Step execution UI
  IterationTimeline.swift       — Wave iteration history
  WaitingStateCard.swift        — PR limit waiting state
  SetupView.swift               — Install loopflow if missing
  DiagnosticsView.swift         — Logs and connection status
  ScreenshotWindow.swift        — Automated screenshot generation
  ThemePreview.swift            — Color palette preview
  TerminalTestWindow.swift      — Ghostty terminal testing
  AreaTypeahead.swift           — Area path autocomplete
  FlowTypeahead.swift           — Flow name autocomplete
  DirectionTypeahead.swift      — Direction/persona autocomplete
  TypeaheadComponents.swift     — Shared typeahead primitives
```

---

## Symphonia (Placeholder)

```swift
// swift/Symphonia/SymphoniaApp.swift — @main, WindowGroup { PlaceholderView() }
// swift/Symphonia/Views/PlaceholderView.swift — "Coming soon" placeholder
```

---

## Tests

```
swift/ConcertoTests/
  AuthServiceTests.swift        — OAuth flow, token refresh, keychain
  GhosttyTests.swift            — Terminal integration
  RunStoreTests.swift           — Wave run caching
  WaveStoreTests.swift          — Grouping, optimistic updates, rollback
  WaveTests.swift               — Model serialization
  WaveRowTests.swift            — UI component (ViewInspector)

swift/ConcertoUITests/
  ScreenshotPipelineTests.swift — Automated screenshot generation

swift/SymphoniaTests/
  SymphoniaTests.swift
```

---

## Communication Architecture

Two channels to lfd daemon:

1. **HTTP API** (`LocalWaveService`) — CRUD waves, runs, flows. Port 2486, base `/v0`.
2. **WebSocket** (`LocalEventService`) — live events at `ws://127.0.0.1:2486/ws`. Auto-reconnect with backoff.

**Optimistic UI:** `WaveStore` tracks pending mutations. Apply change immediately, commit on API success, rollback on error. WebSocket events skip waves with pending mutations.

**Output dual-path:** WebSocket pushes `output_line` events -> `OutputBuffer.appendOutput()`. When wave selected, `OutputBuffer.startStreaming()` connects via HTTP streaming (replay + follow) and suppresses WebSocket output for that wave.

## Key Patterns

1. **Swift 6 concurrency** — `@MainActor`, `actor`, `async/await`, `Sendable` throughout
2. **Observation framework** — `@Observable` (not Combine), `@MainActor` isolation
3. **Dictionary-keyed state** — WaveStore uses `[String: WaveViewModel]` with derived `ordered` and `groups` recomputed on every mutation
4. **Optimistic mutations** — Snapshot -> mutate -> API call -> commit/rollback. Pending set prevents WebSocket overwrites
5. **Environment-based theming** — `LoopflowPalette` resolved from appearance, injected via `@Environment(\.palette)`
6. **Embedded terminal** — libghostty C API wrapped via GhosttyManager singleton, conditionally compiled `GHOSTTY_ENABLED`
7. **Manual JSON parsing** — `JSONSerialization` for API responses, not `Codable`
8. **Wave grouping** — Automatic: blocked > pr > recentActivity > active > idle
9. **macOS notifications** — UNUserNotification for wave state changes (waiting, failed, PR ready)

## Constants

| Constant | Value |
|---|---|
| lfd port | `2486` |
| lfd API base | `http://127.0.0.1:2486/v0` |
| WebSocket | `ws://127.0.0.1:2486/ws` |
| Log directory | `~/Library/Logs/Concerto/` |
| Setup log | `~/.lf/logs/concerto-setup.log` |
| Keychain service | `studio.loopflow.auth` |
| Auth base URL | `https://loopflow.studio` |
| Burgundy | `#722F37` |
| Cream | `#FAF8F5` |
| Terminal bg | `#4A1A2C` |
| Terminal fg | `#F5E6D3` |
| Max output lines | 2000 per wave |
| Max runs cached | 50 per wave |
| Max recent repos | 10 |
| Max recent areas | 5 per repo |
| Default PR limit | 5 |
