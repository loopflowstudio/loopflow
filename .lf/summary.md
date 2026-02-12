# swift/ — Concerto, Symphonia, LoopflowCore

macOS 15+ SwiftUI apps for managing AI coding waves. Three SPM targets sharing a core library.

## Package Structure

```
swift/
  Package.swift          # Swift 6.0, macOS 15+
  LoopflowCore/          # Shared library: models + services
  Concerto/              # Main app: wave management UI
  Symphonia/             # Teams product (placeholder)
  ConcertoTests/
  SymphoniaTests/
  ConcertoUITests/
```

**Dependencies:** ViewInspector (test), GhosttyKit (binary xcframework for embedded terminal)

**Build:** `./dev run` (build+launch), `./dev test`, `./dev release` (dmg)

## LoopflowCore — Shared Models & Services

### Constants

```swift
// LoopflowCore/LoopflowCore.swift
public let lfdDefaultPort = 2486
public let lfdBaseURL = URL(string: "http://127.0.0.1:2486")!
public let lfdApiBaseURL = lfdBaseURL.appendingPathComponent("v0")
```

### Wave (LoopflowCore/Models/Wave.swift)

Central domain object. An autonomous AI coding wave with stimulus, status, and iteration tracking.

```swift
public struct Wave: Sendable, Identifiable, Hashable {
    public let id: String
    public var name: String
    public var repo: String
    public var flow: String              // flow name (e.g. "ship", "debug", "polish")
    public var direction: [String]       // persona directions (e.g. ["product-engineer"])
    public var area: [String]            // code area paths (e.g. ["src/auth"])
    public var stimuli: [Stimulus]       // active triggers (loop/watch/cron)
    public var status: WaveStatus        // idle | running | waiting | failed | paused
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

### Stimulus (LoopflowCore/Models/Wave.swift)

Determines when a wave runs. Stored in stimuli table.

```swift
public struct Stimulus: Sendable, Hashable, Codable, Identifiable {
    public enum Kind: String, Sendable, Codable, CaseIterable {
        case loop   // continuous
        case watch  // on file change
        case cron   // scheduled
    }
    public var id: String
    public var kind: Kind
    public var cron: String?             // cron expression for .cron kind
}
```

### WaveStatus / WaitingReason (LoopflowCore/Models/Wave.swift)

```swift
public enum WaveStatus: String, Sendable, Codable {
    case idle, running, waiting, failed, paused
    public var color: Color { ... }      // semantic status colors
    public var icon: String { ... }      // SF Symbol names
}

public enum WaitingReason: Sendable, Hashable {
    case prLimitReached(open: Int, limit: Int)
}

public enum MergeMode: String, Sendable, Codable {
    case pr, land
}
```

### WaveRun (LoopflowCore/Models/WaveRun.swift)

A single execution of a Wave.

```swift
public struct WaveRun: Sendable, Identifiable, Hashable {
    public let id: String
    public let waveId: String?
    public let flow: String
    public let area: String
    public let repo: String
    public let direction: [String]
    public var status: WaveRunStatus     // pending | running | waiting | completed | failed | cancelled
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
    public var duration: String?         // computed: "3m05s"
    public var relativeTime: String      // computed: "2h ago"
}
```

### Step / Flow (LoopflowCore/Models/Step.swift, Flow.swift)

```swift
public struct Step: Sendable, Codable, Equatable, Identifiable {
    public var prompt: String            // step/prompt name
    public var config: StepConfig?       // optional overrides
}

public struct StepConfig: Sendable, Codable, Equatable {
    public var model: String?
    public var direction: String?
    public var context: [String]?
}

public struct StepRun: Sendable, Identifiable, Codable, Hashable {
    public let id: String
    public let step: String              // step name
    public let repo: String
    public let worktree: String
    public let status: String            // "running" | "waiting" | "completed" | "error"
    public let startedAt: Date
    public let endedAt: Date?
    public let model: String
    public let runMode: String
}

public struct Flow: Sendable, Codable, Identifiable, Equatable {
    public var name: String
    public var steps: [Step]
    public var type: FlowType            // .flow | .step
}
```

### PullRequest (LoopflowCore/Models/PullRequest.swift)

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

### WaveViewModel (LoopflowCore/Models/WaveViewModel.swift)

View-layer wrapper around `Wave` with git state and display helpers.

```swift
public struct WaveViewModel: Sendable, Identifiable, Hashable {
    public var api: Wave                 // underlying Wave data
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
    public var prLimit: Int              // default 5
    public var mergeMode: MergeMode      // default .pr
    public var pid: Int?
    public var lastMainSha: String?
    public var waitingReason: WaitingReason?
    public var runStartedAt: Date?

    // Computed helpers
    public var displayName: String       // name or "area . flow"
    public var statusIndicator: (icon: String, color: Color)
    public var pendingPR: (number: Int, url: URL?)?
    public var hasActiveStimulus: Bool
    public var effectiveOpenPRCount: Int
    public var detailText: String        // "area . flow . stimulus"
}
```

### InteractiveSession / CommitEntry (LoopflowCore/Models/Wave.swift)

```swift
public struct InteractiveSession: Sendable, Identifiable {
    public let id: String
    public let waveId: String
    public let step: String
    public let worktreePath: String
    public let prompt: String?
    public let startedAt: Date
    public var command: String           // computed: "lf <step> '<prompt>'"
}

public struct CommitEntry: Sendable, Hashable, Identifiable {
    public let sha: String
    public let message: String
}
```

### Other Models

```swift
// LoopflowCore/Models/AppPreferences.swift
public enum TerminalApp: String, CaseIterable { case warp, iterm, terminal, kitty }
public enum IDEApp: String, CaseIterable { case cursor, vscode, zed }

// LoopflowCore/Models/AppearanceMode.swift
public enum AppearanceMode: String, CaseIterable { case system, light, dark }

// LoopflowCore/Models/StatusColors.swift
extension Color {
    public static let statusSuccess = Color(hex: 0x2D6A4F)  // green
    public static let statusError = Color(hex: 0xB45309)    // amber
    public static let statusWarning = Color(hex: 0xB0812A)  // gold
    public static let statusInfo = Color(hex: 0x0AB3CC)     // cyan
}
```

---

## LoopflowCore — Services

### WaveServiceProtocol (LoopflowCore/Services/WaveServiceProtocol.swift)

Protocol for testability. `LocalWaveService` is the production implementation.

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
```

### LocalWaveService (LoopflowCore/Services/LocalWaveService.swift)

HTTP client for the `lfd` daemon. All endpoints under `http://127.0.0.1:2486/v0/`.

```swift
public struct LocalWaveService: WaveServiceProtocol, @unchecked Sendable {
    // Short timeout (3s) for fast-fail when daemon not running
    // Long timeout (30s) for git operations (create, land, next, collapse, absorb)

    // MARK: - Waves
    func listWaves(repo: URL) -> [Wave]                    // GET /v0/waves?repo=...&expand[]=active_run
    func getWave(_ id: String) -> Wave                     // GET /v0/waves/{id}?expand[]=active_run
    func createWave(name:, repo:) -> Wave                  // POST /v0/waves
    func updateWave(_ id:, config:) -> Wave                // PATCH /v0/waves/{id}
    func deleteWave(_ id:) -> Void                         // DELETE /v0/waves/{id}
    func cloneWave(_ id:, name:) -> Wave                   // POST /v0/waves/{id}/clone
    func run(_ id:, overrides:) -> Void                    // POST /v0/waves/{id}/run
    func stop(_ id:) -> Void                               // POST /v0/waves/{id}/stop
    func landWave(_ id:) -> Void                           // POST /v0/waves/{id}/land
    func nextWave(_ id:) -> String                         // POST /v0/waves/{id}/next

    // MARK: - Stimulus
    func addStimulus(_ waveId:, kind:, cron:) -> Stimulus  // POST /v0/waves/{id}/stimulus
    func removeStimulus(_ waveId:, stimulusId:) -> Void    // DELETE /v0/waves/{id}/stimulus/{sid}

    // MARK: - Connect (interactive session)
    func connect(_ id:) -> ConnectionInfo                  // POST /v0/waves/{id}/connect

    // MARK: - Flows
    func listFlowsAndDirections(repo:) -> WaveFlowsResult // GET /v0/flows?repo=...

    // MARK: - WaveRuns
    func listWaveRuns(waveId:, repo:, limit:) -> [WaveRun] // GET /v0/wave_runs?...

    // MARK: - PR Operations
    func collapsePRs(_ id:) -> CollapsePRsResult           // POST /v0/waves/{id}/collapse
    func absorbIntoPR(_ id:, prNumber:) -> AbsorbIntoPRResult // POST /v0/waves/{id}/absorb

    // MARK: - Output Streaming
    func streamOutput(waveId:) -> AsyncThrowingStream<String, Error> // GET /v0/waves/{id}/logs (SSE)

    // MARK: - Daemon
    func connectLfd() -> Void                              // shells out: `lfd install`
    func checkAvailability() -> Bool                       // GET /status
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

### LocalEventService (LoopflowCore/Services/LocalEventService.swift)

WebSocket subscription to `ws://127.0.0.1:2486/ws` for live UI updates. Actor-based, auto-reconnects.

```swift
public actor LocalEventService {
    public func subscribe(
        onEvent: @escaping @Sendable (LFDEvent) -> Void,
        onConnectionChange: @escaping @Sendable (Bool) -> Void
    ) async
    public func disconnect() async
}

public enum LFDEvent: Sendable {
    case connected(ConnectedEvent)       // initial wave list on connect
    case wave(WaveEvent)                 // created | updated | deleted | started | stopped | waiting
    case worktree(WorktreeEvent)
    case agentStarted(AgentStartedEvent)
    case agentEnded(AgentEndedEvent)
    case output(OutputEvent)             // live agent output lines
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
    public let wave: Wave?               // full wave data when available
    public let timestamp: Date
}

public struct OutputEvent: Sendable {
    public let waveId: String
    public let agentId: String
    public let text: String
    public let timestamp: Date
}
```

### Auth (LoopflowCore/Services/Auth*.swift, TokenProvider.swift)

```swift
public final class AuthService: NSObject {
    func signIn() async throws -> String           // ASWebAuthenticationSession via loopflow.studio
    func signOut() throws                           // keychain delete
    func currentToken() -> String?                  // keychain read
    func tokenExpiresAt() -> Date?                  // JWT exp decode
    func refreshToken() async throws -> String      // POST /auth/refresh
}

@Observable public final class AuthState {
    public private(set) var token: String?
    public private(set) var isLoading: Bool
    public private(set) var error: AuthError?
    public var isAuthenticated: Bool               // token != nil && !isExpired
    public var isExpired: Bool
    public func signIn() async
    public func signOut()
}

public enum AuthError: Error, Sendable {
    case noCallback, invalidCallback, notAuthenticated, tokenExpired
    case sessionFailed, refreshFailed(String)
    case keychainWrite(OSStatus), keychainDelete(OSStatus), unknown(Error)
}

public protocol TokenProvider: Sendable {
    func token() async throws -> String
}
public struct NoAuthProvider: TokenProvider { ... }
public final class KeychainTokenProvider: TokenProvider { ... }
```

### Other Services

```swift
// LoopflowCore/Services/NotificationService.swift
public final class NotificationService {
    static let shared: NotificationService
    func requestAuthorization() async throws
    func notifyNeedsInteractive(waveId:, waveName:, step:)
    func notifyError(waveId:, waveName:, message:)
    func notifyPRReady(waveId:, waveName:, prNumber:)
}

// LoopflowCore/Services/LoggingService.swift
public enum LoggingService {
    enum Category: String { case worktrees, lfd, general, ui, model }
    static func append(_ message: String, category: Category)
    static func ui(_ message: String)              // shorthand
    static func model(_ message: String)
    static func lfd(_ message: String)
    static func logDirectory() -> URL              // ~/Library/Logs/Concerto/
}
```

---

## Concerto — App & State

### ConcertoApp (Concerto/ConcertoApp.swift)

```swift
@main struct ConcertoApp: App {
    // Scenes:
    //   WindowGroup (default) — WelcomeWindow or ScreenshotWindow or RepoWindow
    //   WindowGroup(id: "repo") — per-repo windows
    //   Window("Terminal Test") — Ghostty test window
    // Global shortcuts: Cmd+K (palette), Cmd+4 (snapshot), Cmd+Shift+T (terminal test)
    // Tint: .loopflowBurgundy
}
```

### RepoState (Concerto/State/RepoState.swift)

Primary observable state. One instance per repo window.

```swift
@MainActor @Observable final class RepoState {
    var currentRepo: URL?
    var flows: [Flow]
    var availableDirections: [String]
    let waveStore: WaveStore
    let runStore: RunStore
    var selectedWaveId: String?
    var selectedWave: WaveViewModel?               // derived from waveStore
    var isLoading: Bool
    var errorMessage: String?
    var lfdConnected: Bool

    // Lifecycle
    func openRepo(_ url: URL, outputBuffer: OutputBuffer) async
    func startEventSubscription(outputBuffer: OutputBuffer)
    func refreshWaves() async
    func refreshFlowsAsync() async

    // Wave CRUD — all use optimistic updates with rollback
    func createWave(name: String) async throws
    func runWave(wave:, area:, direction:, flow:) async throws
    func stopWave(_ wave:) async throws
    func deleteWave(_ wave:) async throws
    func renameWave(_ wave:, to:) async throws
    func updateWave(_ wave:, area:, direction:, flow:, status:) async throws
    func cloneWave(_ wave:) async throws -> WaveViewModel
    func landWave(_ wave:) async throws
    func nextWave(_ wave:) async throws

    // Stimulus
    func addStimulus(wave:, kind:, cron:) async throws
    func removeStimulus(wave:, stimulusId:) async throws

    // PR operations
    func collapsePRs(_ waveId:) async throws -> CollapsePRsResult
    func absorbIntoPR(_ waveId:, prNumber:) async throws -> AbsorbIntoPRResult
    func loadRuns(for waveId:)
    func connectLfd(outputBuffer:) async throws
}
```

**Pattern:** optimistic mutations via `WaveStore.applyOptimistic` + commit/rollback. Actions (run/stop/next) schedule a safety-net refresh after 10s. Real-time updates arrive via WebSocket.

### WaveStore (Concerto/State/WaveStore.swift)

Dictionary-keyed wave state with derived groups and status change tracking.

```swift
@MainActor @Observable final class WaveStore {
    private(set) var waves: [String: WaveViewModel]    // primary storage
    private(set) var ordered: [WaveViewModel]           // derived: groups.allInOrder
    private(set) var groups: WaveGroups                 // derived on any change

    var onStatusChange: ((WaveViewModel, WaveStatus?, WaveStatus) -> Void)?

    func set(_ wave: WaveViewModel)
    func setAll(_ newWaves: [WaveViewModel])
    func remove(_ id: String) -> WaveViewModel?

    // Optimistic mutation protocol
    func applyOptimistic(_ id:, _ mutation:) -> WaveViewModel?  // returns snapshot
    func commitMutation(_ id:)
    func rollback(_ snapshot: WaveViewModel)
    func insertPending(_ wave:)
    func replacePending(_ pendingId:, with:)
    func removePending(_ id:)
    func applyDelete(_ id:)
}

struct WaveGroups {
    let blocked: [WaveViewModel]         // status == .failed
    let pr: [WaveViewModel]              // hasOpenPRs
    let recentActivity: [WaveViewModel]  // activity within last hour (max 5)
    let active: [WaveViewModel]          // running | waiting (not in recent)
    let idle: [WaveViewModel]            // idle (not in recent)
    var attentionCount: Int              // blocked + pr
    var allInOrder: [WaveViewModel]      // concatenated in priority order
}
```

### RunStore (Concerto/State/RunStore.swift)

```swift
@MainActor @Observable final class RunStore {
    private(set) var runs: [String: [WaveRun]]   // keyed by wave ID, max 50 per wave
    func setRuns(for waveId:, _ runs:)
    func runs(for waveId:) -> [WaveRun]
    func clear(for waveId:)
}
```

### OutputBuffer (Concerto/State/OutputBuffer.swift)

Agent output buffering. Supports WebSocket events and HTTP stream replay.

```swift
@MainActor @Observable final class OutputBuffer {
    var interactiveSession: InteractiveSession?

    func appendOutput(waveId:, text:, timestamp:)  // from WebSocket (skipped if stream active)
    func output(for waveId:) -> [OutputLine]
    func clearOutput(for waveId:)
    func startStreaming(waveId:)                    // replay+follow via GET /v0/waves/{id}/logs
    func stopStreaming(waveId:)
    func recentOutput(for waveId:, maxLength:) -> String?
    func launchInteractiveSession(waveId:, step:, worktreePath:, prompt:)
    func endInteractiveSession()
    func hasActiveSession(for waveId:) -> Bool
}

struct OutputLine: Identifiable {
    let id: UUID
    let text: String
    let timestamp: Date
}
```

---

## Concerto — Services

```swift
// Concerto/Services/SetupService.swift
struct SetupService {
    struct DependencyStatus { var lfInstalled: Bool; var lfPath: String? }
    func checkDependencies() -> DependencyStatus
    func install() async throws                    // uv + loopflow + claude code
    func ensureDaemonRunning() async throws        // lfd install / launchctl load
}

// Concerto/Services/TerminalLauncher.swift
struct TerminalLauncher {
    func launchTerminal(_ terminal: TerminalApp, at path: URL, command: String?) throws
    func openInIDE(_ ide: IDEApp, at path: URL, workspace: String?) throws
    func openInFinder(at path: URL)
    func launchStep(_ step: String, terminal: TerminalApp, at repo: URL) throws
}

// Concerto/Services/RecentsService.swift
@Observable final class RecentsService {
    private(set) var recentRepos: [RecentRepo]     // max 10, persisted to UserDefaults
    func addRecent(_ url: URL)
    func removeRecent(_ url: URL)
}

// Concerto/Services/RecentAreasService.swift
struct RecentAreasService {
    func recentAreas(for repoURL: URL) -> [String] // max 5 per repo, UserDefaults
    func addRecentArea(_ area: String, for repoURL: URL)
}

// Concerto/Services/SnapshotService.swift
@MainActor struct SnapshotService {
    func snapshotKeyWindow() throws -> URL         // renders to /tmp/concerto-<timestamp>.png
    func snapshotKeyWindow(to outputPath: String) throws -> URL
}

// Concerto/Services/NameGenerator.swift
enum NameGenerator {
    static func generate() -> String               // "aurora-cadence", "crystal-melody", etc.
}

// Concerto/Services/AppIconProvider.swift
struct AppIconProvider {
    static func icon(for app: AppIdentifier) -> NSImage?
    static func iconImage(for app: AppIdentifier, size: CGFloat) -> Image
}
```

### Ghostty Embedded Terminal (Concerto/Services/Ghostty/)

Embeds Ghostty via `GhosttyKit` xcframework for in-app terminal.

```swift
enum TerminalStatus: Equatable {
    case initializing, running, completed(exitCode: Int32), failed(error: String)
}

struct GhosttySession: Identifiable {
    let id: String
    let worktree: String
    let command: [String]
    var status: TerminalStatus
}

@MainActor final class GhosttyManager: ObservableObject {
    enum State { case uninitialized, initializing, ready, failed(String) }
    @Published private(set) var state: State
    static let shared: GhosttyManager
    var onSessionClosed: (() -> Void)?
    // Wraps libghostty C API (ghostty_app_t, ghostty_surface_t, ghostty_config_t)
}
```

---

## Concerto — Design System

### Brand Colors (Concerto/BrandColors.swift)

```swift
Color.loopflowBurgundy       // #722F37 — accent
Color.loopflowBurgundyHover  // #8B3D47
Color.loopflowCream           // #FAF8F5 — light background
Color.loopflowCreamElevated   // #FFFDFB
Color.loopflowCreamMuted      // #F3EEE7
Color.loopflowSlate            // #2B3036 — dark background
Color.loopflowSlateElevated   // #343B44
Color.loopflowSlateMuted      // #3C4550
Color.loopflowText            // #1A1A1A
Color.loopflowTextSecondary   // #6B6B6B
Color.loopflowTextLight       // #F5F1EA
Color.loopflowInfo            // #0AB3CC

struct LoopflowPalette {       // light/dark adaptive palette
    let background, surface, surfaceMuted, border: Color
    let text, textSecondary, accent, accentHover: Color
    static func make(for scheme: ColorScheme) -> LoopflowPalette
}
```

### Design Tokens (Concerto/DesignSystem.swift)

```swift
enum Spacing     { xxs=2, xs=4, sm=8, md=12, lg=16, xl=20, xxl=24, xxxl=32 }
enum HitTarget   { minimum=24, comfortable=32, touch=44 }
enum CornerRadius { sm=4, md=8, lg=12, xl=16, full=9999 }
enum ZIndex      { base=0, dropdown=100, modal=200, toast=300, tooltip=400 }

enum Typography {
    static let serifFamily = "Cormorant Garamond"   // headlines
    static let sansFamily = "Lato"                   // body, UI
    static let monoFamily = "JetBrains Mono"         // code
    static func heroTitle(_ size: CGFloat = 32) -> Font
    static func sectionTitle(_ size: CGFloat = 20) -> Font
    static func body(_ size: CGFloat = 14) -> Font
    static func code(_ size: CGFloat = 13) -> Font
}

enum DesignAnimation {
    static func standard(_ reduceMotion: Bool) -> Animation?  // 0.2s easeInOut
    static func fast(_ reduceMotion: Bool) -> Animation?      // 0.1s easeOut
    static func spring(_ reduceMotion: Bool) -> Animation?    // spring(0.3, 0.7)
}

// View modifiers
extension View {
    func accessibleButton(_ label: String, hint: String?) -> some View
    func accessibleToggle(_ label: String, isOn: Bool) -> some View
    func minHitTarget() -> some View                 // 24x24 minimum
    func keyboardFocusRing(_ isFocused: Bool) -> some View
}
```

### Flags (Concerto/Flags.swift)

```swift
enum Flags {
    static var beta: Bool     // UserDefaults "beta" key
    static func setBeta(_ enabled: Bool)
}
```

---

## Concerto — Views

All views are in `Concerto/Views/`. Key structure:

| View | File | Role |
|------|------|------|
| `WelcomeWindow` | WelcomeWindow.swift | Launch screen, repo picker |
| `RepoWindow` | RepoWindow.swift | Per-repo window with setup check |
| `ContentView` | ContentView.swift | NavigationSplitView: sidebar + detail |
| `WaveSidebar` | WaveSidebar.swift | Wave list grouped by WaveGroups |
| `WaveRow` | WaveRow.swift | Single wave in sidebar |
| `WaveDetailPanel` | WaveDetailPanel.swift | Current tab + Runs tab for selected wave |
| `WaveRunsTab` | WaveRunsTab.swift | Historical run list with PR state |
| `StepRunner` | StepRunner.swift | Step execution UI |
| `LiveOutput` | LiveOutput.swift | Streaming agent output display |
| `CommandPalette` | CommandPalette.swift | Cmd+K fuzzy action search |
| `FlowTypeahead` | FlowTypeahead.swift | Flow/step picker |
| `AreaTypeahead` | AreaTypeahead.swift | Code area picker |
| `DirectionTypeahead` | DirectionTypeahead.swift | Direction/persona picker |
| `TypeaheadComponents` | TypeaheadComponents.swift | Shared typeahead building blocks |
| `FlowProgressPills` | FlowProgressPills.swift | Step progress indicators |
| `IterationTimeline` | IterationTimeline.swift | Iteration history |
| `NextActionsBar` | NextActionsBar.swift | Land / Next buttons |
| `WaitingStateCard` | WaitingStateCard.swift | Waiting reason display |
| `SetupView` | SetupView.swift | First-run dependency install |
| `QuickExperimentView` | QuickExperimentView.swift | Quick one-off step launch |
| `InteractiveSessionView` | InteractiveSessionView.swift | Embedded terminal session |
| `EmbeddedTerminalPanel` | EmbeddedTerminalPanel.swift | Ghostty terminal wrapper |
| `TerminalTestWindow` | TerminalTestWindow.swift | Standalone Ghostty test |
| `ScreenshotWindow` | ScreenshotWindow.swift | Automated screenshot capture |
| `DiagnosticsView` | DiagnosticsView.swift | Debug/diagnostics panel |

---

## Concerto — Communication with lfd

Two patterns, intentionally different:

1. **HTTP** (`LocalWaveService`) — CRUD operations, wave actions (run/stop/land/next), output streaming via SSE
2. **WebSocket** (`LocalEventService` at `ws://127.0.0.1:2486/ws`) — live event subscription for real-time UI updates

Events flow: WebSocket -> RepoState.handleWaveEvent -> WaveStore.set -> derived groups recompute -> SwiftUI re-renders

---

## Symphonia (swift/Symphonia/)

Placeholder app for team coordination. Currently just a `PlaceholderView`. Depends on `LoopflowCore`.

---

## Key Patterns

- **Optimistic updates:** `WaveStore` applies mutations immediately, commits on API success, rolls back on failure. Pending mutations block server-pushed updates to avoid flicker.
- **Observable macro:** All state uses Swift `@Observable` (not `ObservableObject`). Views access state via `@Environment`.
- **Actor isolation:** `LocalEventService` is an actor. `RepoState`, `WaveStore`, `RunStore`, `OutputBuffer` are `@MainActor`.
- **Sendable throughout:** All models are `Sendable`. Services use `@unchecked Sendable` where needed for URLSession.
- **Per-repo windows:** Each `RepoWindow` creates its own `RepoState` + `OutputBuffer` instances.
- **Design system enforcement:** `Spacing`, `HitTarget`, `CornerRadius`, `Typography`, `DesignAnimation` enums prevent arbitrary values.
- **Accessibility:** `reduceMotion` checked for all animations. `accessibleButton`/`accessibleToggle` modifiers for controls.
