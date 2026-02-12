# Swift Codebase Summary

Package `LoopflowSwift` — macOS 15+, Swift 6.0. Two apps + shared library.

```
swift/
├── Package.swift
├── LoopflowCore/          # Shared library (models + services)
│   ├── LoopflowCore.swift # Config constants
│   ├── Models/            # Wave, WaveRun, Flow, Step, PullRequest, etc.
│   └── Services/          # LocalWaveService, LocalEventService, Auth, Logging, Notifications
├── Concerto/              # macOS desktop app (SwiftUI)
│   ├── ConcertoApp.swift  # @main entry, multi-window
│   ├── BrandColors.swift  # Color palette + LoopflowPalette
│   ├── DesignSystem.swift # Spacing, Typography, CornerRadius, HitTarget, ZIndex
│   ├── Flags.swift        # Beta feature flag
│   ├── ScriptCommands.swift # AppleScript screenshot handler
│   ├── Models/            # RecentRepo
│   ├── Services/          # Ghostty terminal, NameGenerator, RecentsService, etc.
│   ├── State/             # RepoState, WaveStore, RunStore, OutputBuffer
│   └── Views/             # ~25 SwiftUI views
├── Symphonia/             # iOS app (placeholder)
├── ConcertoTests/         # Unit tests (ViewInspector)
├── ConcertoUITests/       # Screenshot pipeline
└── SymphoniaTests/
```

## Dependencies

- **ViewInspector** 0.10.0+ (test-only, SwiftUI view testing)
- **GhosttyKit** (binary xcframework, embedded terminal)
- System: Carbon, QuartzCore, Metal, IOKit, libc++
- Build define: `GHOSTTY_ENABLED` (Concerto only)

## Configuration

```swift
// LoopflowCore/LoopflowCore.swift
public let lfdDefaultPort = 2486
public let lfdBaseURL = URL(string: "http://127.0.0.1:2486")!
public let lfdApiBaseURL = lfdBaseURL.appendingPathComponent("v0")
```

---

## LoopflowCore — Models

### Wave (`LoopflowCore/Models/Wave.swift`)

Central domain object — an autonomous AI coding wave that runs flows on a repo.

```swift
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
    // .color: Color — maps to status design tokens
    // .icon: String — SF Symbol name
}

public struct Stimulus: Sendable, Hashable, Codable, Identifiable {
    public var id: String
    public var kind: Kind
    public var cron: String?

    public enum Kind: String, Sendable, Codable, CaseIterable {
        case loop   // Continuous
        case watch  // On file change
        case cron   // Scheduled
    }
}

public enum MergeMode: String, Sendable, Codable { case pr, land }

public enum WaitingReason: Sendable, Hashable {
    case prLimitReached(open: Int, limit: Int)
}

public struct CommitEntry: Sendable, Hashable, Identifiable {
    public let sha: String   // id = sha
    public let message: String
}

public struct InteractiveSession: Sendable, Identifiable {
    public let id: String
    public let waveId: String
    public let step: String
    public let worktreePath: String
    public let prompt: String?
    public let startedAt: Date
    // .command: String — builds "lf <step> <prompt>"
}

public func shellEscape(_ string: String) -> String  // wraps in single quotes
```

### WaveRun (`LoopflowCore/Models/WaveRun.swift`)

Single execution of a Wave.

```swift
public struct WaveRun: Sendable, Identifiable, Hashable {
    public let id: String
    public let waveId: String?
    public let flow: String
    public let area: String       // note: String, not [String]
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
    // .duration: String? — "Xm XXs"
    // .relativeTime: String — abbreviated relative time
}

public enum WaveRunStatus: String, Sendable, Codable {
    case pending, running, waiting, completed, failed, cancelled
}
```

### WaveViewModel (`LoopflowCore/Models/WaveViewModel.swift`)

Rich view model wrapping `Wave` with computed UI properties.

```swift
public struct WaveViewModel: Sendable, Identifiable, Hashable {
    public var api: Wave                // Underlying API model
    public var worktreePath: String?    // Resolved: activeRun?.worktree ?? api.localWorktree
    public var branch: String?          // Resolved: activeRun?.branch ?? api.remoteBranch
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
    public var prLimit: Int             // default 5
    public var mergeMode: MergeMode     // default .pr
    public var pid: Int?
    public var lastMainSha: String?
    public var waitingReason: WaitingReason?
    public var runStartedAt: Date?
}
```

Key computed properties:
- `id`, `name`, `repo`, `flow`, `direction`, `area`, `stimuli`, `status`, `iteration`, `activeRun`, `createdAt` — delegate to `api`
- `displayName` — name or "area · flow"
- `statusIndicator` — `(icon: String, color: Color)` tuple
- `pendingPR` — `(number: Int, url: URL?)?` when prState == .open
- `detailText` — "area · flow · stimulus"
- `hasOpenPRs`, `effectiveOpenPRCount`, `hasActiveStimulus`
- `lastActivityAt`, `lastActivityDescription`

### Flow & Step (`LoopflowCore/Models/Flow.swift`, `Step.swift`)

```swift
public enum FlowType: String, Sendable, Codable { case flow, step }

public struct Flow: Sendable, Codable, Identifiable, Equatable {
    public var id: UUID
    public var name: String
    public var steps: [Step]
    public var type: FlowType   // .flow or .step
}

public struct Step: Sendable, Codable, Equatable, Identifiable {
    public var id: UUID
    public var prompt: String
    public var config: StepConfig?
}

public struct StepConfig: Sendable, Codable, Equatable {
    public var model: String?
    public var direction: String?
    public var context: [String]?
}

public struct StepRun: Sendable, Identifiable, Codable, Hashable {
    public let id: String
    public let step: String
    public let repo: String
    public let worktree: String
    public let status: String     // "running", "waiting", "completed", "error"
    public let startedAt: Date
    public let endedAt: Date?
    public let model: String
    public let runMode: String
    // .isRunning, .isCompleted, .isError — computed
}
```

### PullRequest (`LoopflowCore/Models/PullRequest.swift`)

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

### Preferences (`LoopflowCore/Models/AppPreferences.swift`)

```swift
public enum TerminalApp: String, Sendable, CaseIterable {
    case warp, iterm, terminal, kitty
}
public enum IDEApp: String, Sendable, CaseIterable {
    case cursor, vscode, zed
}
```

### Appearance (`LoopflowCore/Models/AppearanceMode.swift`, `StatusColors.swift`)

```swift
public enum AppearanceMode: String, Sendable, CaseIterable {
    case system, light, dark
}

// Color extensions (StatusColors.swift):
Color.statusSuccess  = 0x2D6A4F  // green
Color.statusError    = 0xB45309  // amber/orange
Color.statusWarning  = 0xB0812A  // gold
Color.statusInfo     = 0x0AB3CC  // cyan
Color.statusNeutral  = 0x8B8B8B  // gray
```

---

## LoopflowCore — Services

### WaveServiceProtocol (`Services/WaveServiceProtocol.swift`)

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

### LocalWaveService (`Services/LocalWaveService.swift`)

HTTP client for `lfd` daemon. Implements `WaveServiceProtocol`.

```swift
public struct LocalWaveService: WaveServiceProtocol, @unchecked Sendable {
    // Two URLSessions: default (3s timeout) and longSession (30s) for git ops
    // All endpoints under lfdApiBaseURL (/v0/...)

    // Additional methods beyond protocol:
    func listWaveRuns(waveId: String?, repo: URL?, limit: Int = 50) async throws -> [WaveRun]
    func streamOutput(waveId: String) -> AsyncThrowingStream<String, Error>
    func connectLfd() async throws           // runs "lfd install"
    func checkAvailability() async -> Bool    // GET /status
    func connect(_ id: String) async throws -> ConnectionInfo
    func collapsePRs(_ id: String) async throws -> CollapsePRsResult
    func absorbIntoPR(_ id: String, prNumber: Int) async throws -> AbsorbIntoPRResult

    // JSON parsing (public static for reuse by LocalEventService):
    static func parseWaveFromJSON(_ json: [String: Any]) -> Wave
}
```

API endpoints:
| Method | Path | Description |
|--------|------|-------------|
| GET | `/v0/waves?repo=&expand[]=active_run` | List waves |
| GET | `/v0/waves/{id}?expand[]=active_run` | Get wave |
| POST | `/v0/waves` | Create wave |
| PATCH | `/v0/waves/{id}` | Update wave config |
| DELETE | `/v0/waves/{id}` | Delete wave |
| POST | `/v0/waves/{id}/run` | Run wave (with optional overrides) |
| POST | `/v0/waves/{id}/stop` | Stop wave |
| POST | `/v0/waves/{id}/land` | Land wave (create PR) |
| POST | `/v0/waves/{id}/next` | Next iteration |
| POST | `/v0/waves/{id}/clone` | Clone wave |
| POST | `/v0/waves/{id}/connect` | Connect to running agent |
| POST | `/v0/waves/{id}/stimulus` | Add stimulus |
| DELETE | `/v0/waves/{id}/stimulus/{sid}` | Remove stimulus |
| POST | `/v0/waves/{id}/collapse` | Collapse PRs |
| POST | `/v0/waves/{id}/absorb` | Absorb into PR |
| GET | `/v0/waves/{id}/logs` | Stream output (replay + follow) |
| GET | `/v0/wave_runs?wave_id=&repo=&limit=` | List runs |
| GET | `/v0/flows?repo=` | List flows and directions |
| GET | `/status` | Health check |

Helper types:

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
```

### LocalEventService (`Services/LocalEventService.swift`)

WebSocket client for real-time lfd events at `ws://127.0.0.1:2486/ws`.

```swift
public actor LocalEventService {
    public func subscribe(
        onEvent: @escaping @Sendable (LFDEvent) -> Void,
        onConnectionChange: @escaping @Sendable (Bool) -> Void
    ) async
    public func disconnect() async
    public var isConnected: Bool
    // Auto-reconnects: 1s for first 10 attempts, then 5s
}

public enum LFDEvent: Sendable {
    case connected(ConnectedEvent)
    case wave(WaveEvent)
    case worktree(WorktreeEvent)
    case agentStarted(AgentStartedEvent)
    case agentEnded(AgentEndedEvent)
    case output(OutputEvent)
}

public struct ConnectedEvent: Sendable {
    public let timestamp: Date
    public let waves: [Wave]           // Full wave list on connect
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
    public let wave: Wave?              // Full wave payload (optional)
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
```

WebSocket message types: `connected`, `ping`, `wave_created`, `wave_updated`, `wave_deleted`, `wave_started`, `wave_stopped`, `wave_waiting`, `worktree_updated`, `agent_started`, `agent_ended`, `output_line`.

### AuthService (`Services/AuthService.swift`, `AuthError.swift`, `AuthState.swift`)

OAuth via `loopflow.studio` with keychain storage.

```swift
public final class AuthService: NSObject, @unchecked Sendable {
    // Keychain: service="studio.loopflow.auth", account="jwt"
    @MainActor func signIn() async throws -> String  // ASWebAuthenticationSession
    func signOut() throws
    func currentToken() -> String?
    func tokenExpiresAt() -> Date?
    func refreshToken() async throws -> String  // POST /auth/refresh
    static func decodeExpiry(_ token: String) -> Date?  // JWT exp claim
}

public enum AuthError: Error, Sendable, LocalizedError {
    case noCallback, invalidCallback, notAuthenticated, tokenExpired
    case sessionFailed, refreshFailed(String)
    case keychainWrite(OSStatus), keychainDelete(OSStatus)
    case unknown(Error)
}

@MainActor @Observable
public final class AuthState {
    public private(set) var token: String?
    public private(set) var isLoading: Bool
    public private(set) var error: AuthError?
    public var isAuthenticated: Bool   // token != nil && !isExpired
    public var isExpired: Bool
    public var needsRefresh: Bool      // expires within 24h
    public func signIn() async
    public func signOut()
    // Hourly refresh monitor runs in background
}

public protocol TokenProvider: Sendable {
    func token() async throws -> String
}
public struct NoAuthProvider: TokenProvider { ... }
public final class KeychainTokenProvider: TokenProvider { ... }
```

### LoggingService (`Services/LoggingService.swift`)

```swift
public enum LoggingService {
    public enum Category: String { case worktrees, lfd, general, ui, model }
    // Logs to ~/Library/Logs/Concerto/<category>.log
    public static func append(_ message: String, category: Category = .worktrees)
    public static func ui(_ message: String)
    public static func model(_ message: String)
    public static func lfd(_ message: String)
    public static func read(category: Category) -> String
    public static func logPath(category: Category) -> String
}
```

### NotificationService (`Services/NotificationService.swift`)

```swift
public final class NotificationService: NSObject, UNUserNotificationCenterDelegate {
    public static let shared: NotificationService
    public func requestAuthorization() async throws
    public func notifyNeedsInteractive(waveId: String, waveName: String, step: String)
    public func notifyError(waveId: String, waveName: String, message: String)
    public func notifyPRReady(waveId: String, waveName: String, prNumber: Int)
}

extension Notification.Name {
    public static let selectWave = Notification.Name("selectWave")
}
```

---

## Concerto App — State Management

### RepoState (`Concerto/State/RepoState.swift`)

Primary observable state. Owns `WaveStore`, `RunStore`. Manages lfd connection, WebSocket events.

```swift
@MainActor @Observable
final class RepoState {
    var currentRepo: URL?
    var flows: [Flow]
    var availableDirections: [String]
    let waveStore = WaveStore()
    let runStore = RunStore()
    var waves: [WaveViewModel] { waveStore.ordered }        // derived
    var waveGroups: WaveGroups { waveStore.groups }          // derived
    var selectedWaveId: String?
    var selectedWave: WaveViewModel?                          // derived from selectedWaveId
    private(set) var inFlightActions: Set<String>
    var isLoading: Bool
    var errorMessage: String?
    var lfdConnected: Bool

    // Lifecycle
    func openRepo(_ url: URL, outputBuffer: OutputBuffer, skipBackgroundRefresh: Bool = false) async
    func refreshWaves() async
    func refreshFlowsAsync() async
    func connectLfd(outputBuffer: OutputBuffer) async throws

    // Wave CRUD (all use optimistic updates with rollback)
    func createWave(name: String) async throws
    func deleteWave(_ wave: WaveViewModel) async throws
    func renameWave(_ wave: WaveViewModel, to newName: String) async throws
    func updateWave(_ wave: WaveViewModel, area/direction/flow/status) async throws
    func cloneWave(_ wave: WaveViewModel) async throws -> WaveViewModel

    // Wave actions
    func runWave(wave: WaveViewModel, area/direction/flow overrides) async throws
    func stopWave(_ wave: WaveViewModel) async throws
    func landWave(_ wave: WaveViewModel) async throws
    func nextWave(_ wave: WaveViewModel) async throws

    // Stimulus
    func addStimulus(wave: WaveViewModel, kind: Stimulus.Kind, cron: String?) async throws
    func removeStimulus(wave: WaveViewModel, stimulusId: String) async throws

    // PR ops
    func collapsePRs(_ waveId: String) async throws -> CollapsePRsResult
    func absorbIntoPR(_ waveId: String, prNumber: Int) async throws -> AbsorbIntoPRResult

    // Test/screenshot modes
    static func uiTestMode() -> UITestMode?
    func configureForUITest(_ mode: UITestMode, repoURL: URL)
    func configureMockWaves()
}

// Screenshot mode args: --snapshot <path> --repo <path> --select <branch> --tab <tab>
//                       --size WxH --mock-loops --mock-config <path>
```

### WaveStore (`Concerto/State/WaveStore.swift`)

Dictionary-backed wave storage with derived grouping.

```swift
@MainActor @Observable
final class WaveStore {
    private(set) var waves: [String: WaveViewModel]  // didSet triggers recompute()
    private(set) var ordered: [WaveViewModel]        // derived
    private(set) var groups: WaveGroups              // derived
    var onStatusChange: ((WaveViewModel, WaveStatus?, WaveStatus) -> Void)?

    func set(_ wave: WaveViewModel)                  // skips if pending mutation
    func setAll(_ newWaves: [WaveViewModel])         // preserves pending mutations
    func remove(_ id: String) -> WaveViewModel?
    func removeAll()

    // Optimistic updates
    func applyOptimistic(_ id: String, _ mutation: (inout WaveViewModel) -> Void) -> WaveViewModel?
    func commitMutation(_ id: String)
    func rollback(_ snapshot: WaveViewModel)

    // Pending create/delete
    func insertPending(_ wave: WaveViewModel)
    func replacePending(_ pendingId: String, with wave: WaveViewModel)
    func removePending(_ id: String)
    func applyDelete(_ id: String)

    func wave(for id: String) -> WaveViewModel?
}

struct WaveGroups {
    let blocked: [WaveViewModel]          // status == .failed
    let pr: [WaveViewModel]               // hasOpenPRs (non-failed)
    let recentActivity: [WaveViewModel]   // lastActivityAt within 1 hour (max 5)
    let active: [WaveViewModel]           // running/waiting, not recent
    let idle: [WaveViewModel]             // idle, not recent
    var attentionCount: Int               // blocked + pr count
    var openPRCount: Int                  // sum of effectiveOpenPRCount
    var allInOrder: [WaveViewModel]       // blocked + pr + recent + active + idle
}
```

### RunStore (`Concerto/State/RunStore.swift`)

```swift
@MainActor @Observable
final class RunStore {
    private(set) var runs: [String: [WaveRun]]  // keyed by wave ID, max 50 per wave
    func setRuns(for waveId: String, _ newRuns: [WaveRun])
    func runs(for waveId: String) -> [WaveRun]
    func clear(for waveId: String)
}
```

### OutputBuffer (`Concerto/State/OutputBuffer.swift`)

```swift
struct OutputLine: Identifiable {
    let id: UUID
    let text: String
    let timestamp: Date
}

@MainActor @Observable
final class OutputBuffer {
    var interactiveSession: InteractiveSession?  // one at a time
    // maxLines = 2000

    func appendOutput(waveId: String, text: String, timestamp: Date)  // skips if stream active
    func output(for waveId: String) -> [OutputLine]
    func clearOutput(for waveId: String)
    func startStreaming(waveId: String)    // replay + follow via /v0/waves/{id}/logs
    func stopStreaming(waveId: String)
    func recentOutput(for waveId: String, maxLength: Int = 60) -> String?
    func launchInteractiveSession(waveId: String, step: String, worktreePath: String, prompt: String?)
    func endInteractiveSession()
    func hasActiveSession(for waveId: String) -> Bool
}
```

---

## Concerto App — Design System

### Brand Colors (`Concerto/BrandColors.swift`)

```swift
Color.loopflowBurgundy       = 0x722F37
Color.loopflowBurgundyHover  = 0x8B3D47
Color.loopflowCream           = 0xFAF8F5
Color.loopflowCreamElevated   = 0xFFFDFB
Color.loopflowCreamMuted      = 0xF3EEE7
Color.loopflowSlate           = 0x2B3036
Color.loopflowSlateElevated   = 0x343B44
Color.loopflowSlateMuted      = 0x3C4550
Color.loopflowText            = 0x1A1A1A
Color.loopflowTextSecondary   = 0x6B6B6B
Color.loopflowTextLight       = 0xF5F1EA
Color.loopflowTextSecondaryLight = 0xC8C1B8
Color.loopflowInfo            = 0x0AB3CC

struct LoopflowPalette {
    let background, surface, surfaceMuted, border: Color
    let text, textSecondary, accent, accentHover: Color
    static func make(for scheme: ColorScheme) -> LoopflowPalette
    // Dark: slate backgrounds, light text, burgundy accent
    // Light: cream backgrounds, dark text, burgundy accent
}
```

### Design Tokens (`Concerto/DesignSystem.swift`)

```swift
enum Spacing     { xxs=2, xs=4, sm=8, md=12, lg=16, xl=20, xxl=24, xxxl=32 }
enum HitTarget   { minimum=24, comfortable=32, touch=44 }
enum ZIndex      { base=0, dropdown=100, modal=200, toast=300, tooltip=400 }
enum CornerRadius { sm=4, md=8, lg=12, xl=16, full=9999 }

enum Typography {
    static let serifFamily = "Cormorant Garamond"  // headlines
    static let sansFamily = "Lato"                 // body, UI
    static let monoFamily = "JetBrains Mono"       // code, terminal
    func heroTitle(_ size: CGFloat = 32) -> Font
    func sectionTitle(_ size: CGFloat = 20) -> Font
    func body(_ size: CGFloat = 14) -> Font
    func code(_ size: CGFloat = 13) -> Font
    // + bodyBold, caption, codeSmall
}

enum DesignAnimation {
    func standard(_ reduceMotion: Bool) -> Animation?   // 0.2s ease
    func fast(_ reduceMotion: Bool) -> Animation?       // 0.1s ease
    func spring(_ reduceMotion: Bool) -> Animation?     // 0.3s spring
}

// View modifiers:
.accessibleButton(_ label: String, hint: String?)
.accessibleToggle(_ label: String, isOn: Bool)
.minHitTarget()           // 24x24
.keyboardFocusRing(_ isFocused: Bool)

struct DarkButtonStyle: ButtonStyle  // burgundy bg, cream text
```

---

## Concerto App — Services

### GhosttyManager (`Services/Ghostty/GhosttyManager.swift`)

Embedded terminal via GhosttyKit binary framework.

```swift
@MainActor
final class GhosttyManager: ObservableObject {
    enum State: Equatable { case uninitialized, initializing, ready, failed(String) }
    static let shared: GhosttyManager
    @Published private(set) var state: State
    var onSessionClosed: (() -> Void)?

    func initialize()
    func tick()     // called from display link
    func createSurface(workingDirectory: String, command: String?, view: NSView) -> ghostty_surface_t?
    func destroySurface(_ surface: ghostty_surface_t)
    func registerActiveSession(_ surface: ghostty_surface_t, sessionId: String)
    func destroyActiveSession()
    func sendText(_ text: String)
}
```

### Other Concerto Services

```swift
// TerminalLauncher — open external terminals/IDEs
struct TerminalLauncher {
    func launchTerminal(_ terminal: TerminalApp, at path: URL, command: String?) throws
    func openInIDE(_ ide: IDEApp, at path: URL, workspace: String?) throws
    func openInFinder(at path: URL)
    func openURL(_ url: URL)
    func launchStep(_ step: String, terminal: TerminalApp, at repo: URL) throws
}

// SnapshotService — window screenshots
@MainActor struct SnapshotService {
    func snapshotKeyWindow() throws -> URL
    func snapshotKeyWindow(to outputPath: String) throws -> URL
    func snapshotWindow(_ window: NSWindow, to outputURL: URL) throws -> URL
}

// RecentsService — recent repo tracking
@Observable final class RecentsService {
    private(set) var recentRepos: [RecentRepo]
    func addRecent(_ url: URL)
    func removeRecent(_ url: URL)
    func clearAll()
}

struct RecentRepo: Codable, Identifiable {
    let path: String
    let lastOpened: Date
    var url: URL, displayName: String, exists: Bool  // computed
}

// SetupService — dependency checking
struct SetupService {
    struct DependencyStatus { var lfInstalled: Bool; var lfPath: String? }
    func checkDependencies() -> DependencyStatus
    func install() async throws
    func ensureDaemonRunning() async throws
}

// NameGenerator — random wave names
enum NameGenerator {
    static func generate() -> String  // "aurora-melody" pattern
}

// RecentAreasService — track recently used areas per repo
// Flags — beta feature flag (UserDefaults)
// ScriptCommands — AppleScript "capture screenshot" command
```

---

## Concerto App — View Hierarchy

```
ConcertoApp (@main)
├── WindowGroup (default) → WelcomeWindow or RepoWindow
├── WindowGroup(id: "repo", for: URL.self) → RepoWindow
└── Window("Terminal Test") → TerminalTestWindow

RepoWindow
└── ContentView                        # NavigationSplitView
    ├── WaveSidebar                    # Left panel: grouped wave list
    │   └── WaveRow                    # Per-wave list item
    └── WaveDetailPanel                # Right panel: selected wave
        ├── StepRunner                 # Run step with typeaheads
        │   ├── AreaTypeahead
        │   ├── FlowTypeahead
        │   └── DirectionTypeahead
        ├── InteractiveSessionView     # Embedded Ghostty terminal
        │   └── EmbeddedTerminalPanel
        ├── WaveRunsTab                # Run history
        │   └── IterationTimeline      # Visual PR/merge timeline
        ├── LiveOutput                 # Real-time agent output
        ├── FlowProgressPills          # Step progress indicator
        ├── NextActionsBar             # Suggested actions
        ├── WaitingStateCard           # Blocked state display
        └── QuickExperimentView        # Quick experiment launcher

CommandPalette                         # ⌘K overlay
DiagnosticsView                        # Logs viewer
ScreenshotWindow                       # Screenshot automation
SetupView                              # First-run setup
TypeaheadComponents                    # Shared typeahead building blocks
```

---

## Key Patterns

1. **Observable Architecture** — Swift `@Observable` macro (not Combine). State flows: `RepoState` → `WaveStore` → Views. All `@MainActor`.

2. **Optimistic Updates with Rollback** — `WaveStore.applyOptimistic()` captures snapshot, applies mutation instantly, commits or rolls back after API response. Pending mutations are skipped by `set()`/`setAll()` to prevent WebSocket events from clobbering.

3. **HTTP + WebSocket Hybrid** — `LocalWaveService` for commands (HTTP REST), `LocalEventService` for real-time events (WebSocket). Both parse JSON manually (no Codable for API responses due to snake_case + flexible types).

4. **Wave Grouping** — Priority: blocked (failed) → PR (has open PRs) → recent activity (last hour, max 5) → active (running/waiting) → idle. `WaveGroups.allInOrder` is the display order.

5. **Multi-Window** — `WindowGroup(id: "repo", for: URL.self)` enables per-repo windows with independent state.

6. **Test Modes** — UI test: `-ui-test-mode mock-waves`. Screenshot: `--snapshot <path> --repo <path> --select <branch>`. Mock data in `RepoState.configureMockWaves()`.

7. **Design Token System** — 4pt spacing scale, semantic status colors, LoopflowPalette with light/dark variants, serif/sans/mono typography, accessibility modifiers.

---

## Tests

| File | Coverage |
|------|----------|
| `ConcertoTests/WaveStoreTests.swift` | Wave grouping, optimistic mutations |
| `ConcertoTests/WaveTests.swift` | Wave model, status |
| `ConcertoTests/WaveRowTests.swift` | Wave list item UI |
| `ConcertoTests/RunStoreTests.swift` | Run caching |
| `ConcertoTests/AuthServiceTests.swift` | Auth flows, JWT decoding |
| `ConcertoTests/GhosttyTests.swift` | Terminal integration |
| `ConcertoUITests/ScreenshotPipelineTests.swift` | UI screenshot automation |

Run: `swift test --package-path swift`
