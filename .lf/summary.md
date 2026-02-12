# Swift Codebase Summary

Three SPM targets: **LoopflowCore** (shared library), **Concerto** (macOS app), **Symphonia** (placeholder).
Platform: macOS 15+. Swift tools version 6.0. Strict concurrency.

## Directory Structure

```
swift/
├── Package.swift                   # SPM config
├── LoopflowCore/                   # Shared models + services
│   ├── LoopflowCore.swift          # Constants (lfdDefaultPort, lfdBaseURL, lfdApiBaseURL)
│   ├── Models/
│   │   ├── Wave.swift              # Wave, Stimulus, WaveStatus, CommitEntry, InteractiveSession
│   │   ├── WaveRun.swift           # WaveRun, WaveRunStatus
│   │   ├── WaveViewModel.swift     # WaveViewModel (Wave + enriched git/PR state)
│   │   ├── Step.swift              # Step, StepConfig, StepRun
│   │   ├── Flow.swift              # Flow, FlowType
│   │   ├── PullRequest.swift       # PullRequest, PRState
│   │   ├── StatusColors.swift      # Color.statusSuccess/Error/Warning/Info/Neutral
│   │   ├── AppPreferences.swift    # TerminalApp, IDEApp enums
│   │   └── AppearanceMode.swift    # system/light/dark
│   └── Services/
│       ├── LocalWaveService.swift  # HTTP client for lfd (935 lines)
│       ├── LocalEventService.swift # WebSocket event subscription (actor)
│       ├── WaveServiceProtocol.swift # Protocol + WaveFlowsResult
│       ├── AuthService.swift       # OAuth + keychain JWT
│       ├── AuthState.swift         # Observable auth state
│       ├── AuthError.swift         # Auth error enum
│       ├── TokenProvider.swift     # Protocol: NoAuthProvider, KeychainTokenProvider
│       ├── LoggingService.swift    # File-based logging to ~/Library/Logs/Concerto/
│       └── NotificationService.swift # macOS UNUserNotification for wave events
├── Concerto/                       # macOS app
│   ├── ConcertoApp.swift           # @main entry, WindowGroup scenes
│   ├── BrandColors.swift           # LoopflowPalette, Color extensions, PaletteKey
│   ├── DesignSystem.swift          # Spacing, HitTarget, ZIndex, CornerRadius, Typography
│   ├── Flags.swift                 # Feature flags (beta via UserDefaults)
│   ├── ScriptCommands.swift        # AppleScript handlers
│   ├── Models/RecentRepo.swift
│   ├── State/
│   │   ├── RepoState.swift         # Primary app state (@Observable, 631 lines)
│   │   ├── WaveStore.swift         # Dictionary-keyed wave storage with groups
│   │   ├── OutputBuffer.swift      # Agent output buffering + streaming
│   │   └── RunStore.swift          # Cached wave runs
│   ├── Services/
│   │   ├── NameGenerator.swift     # Random wave names (magical-musical)
│   │   ├── SetupService.swift      # Dependency installation (lf, uv, node, claude)
│   │   ├── SnapshotService.swift   # Window screenshots
│   │   ├── TerminalLauncher.swift  # Launch Warp/iTerm/Terminal/Kitty + Cursor/VSCode/Zed
│   │   ├── RecentsService.swift    # Recent repos persistence
│   │   ├── RecentAreasService.swift
│   │   ├── AppIconProvider.swift
│   │   └── Ghostty/               # Embedded terminal
│   │       ├── GhosttyManager.swift  # libghostty C API wrapper
│   │       ├── GhosttyTerminalView.swift
│   │       └── GhosttyTypes.swift
│   └── Views/                      # 26 SwiftUI view files
│       ├── ContentView.swift       # Main split view (sidebar + detail)
│       ├── RepoWindow.swift        # Window wrapper, setup check
│       ├── WelcomeWindow.swift     # Launch screen
│       ├── WaveSidebar.swift       # Sidebar wave list with groups
│       ├── WaveRow.swift           # Sidebar wave item (262 lines)
│       ├── WaveDetailPanel.swift   # Selected wave config/status
│       ├── WaveRunsTab.swift       # Wave run history
│       ├── LiveOutput.swift        # Agent output display
│       ├── CommandPalette.swift    # Cmd+K command palette
│       ├── NextActionsBar.swift    # Run/stop/land action buttons
│       ├── FlowProgressPills.swift # Step progress indicators
│       ├── IterationTimeline.swift # Wave iteration history
│       ├── WaitingStateCard.swift  # Waiting/interactive prompt
│       ├── EmbeddedTerminalPanel.swift # Ghostty terminal panel
│       ├── InteractiveSessionView.swift
│       ├── StepRunner.swift        # Step execution UI
│       ├── QuickExperimentView.swift
│       ├── ScreenshotWindow.swift  # Automated screenshot capture
│       ├── SetupView.swift         # First-run setup
│       ├── AreaTypeahead.swift     # Area input with autocomplete
│       ├── FlowTypeahead.swift     # Flow picker
│       ├── DirectionTypeahead.swift # Direction picker
│       ├── TypeaheadComponents.swift # Shared typeahead primitives
│       ├── DiagnosticsView.swift
│       ├── ThemePreview.swift
│       └── TerminalTestWindow.swift
├── Symphonia/                      # Teams product (placeholder)
├── ConcertoTests/                  # Unit tests (ViewInspector)
└── ConcertoUITests/                # UI tests (XCTest)
```

## Package.swift

```swift
// swift-tools-version: 6.0
platforms: [.macOS(.v15)]
products: [LoopflowCore (library), Concerto (executable), Symphonia (executable)]
dependencies: [ViewInspector 0.10.0+]
// GhosttyKit: binary xcframework from bin.loopflow.studio
// Concerto linker: Carbon, QuartzCore, Metal, IOKit, libc++
// Concerto swiftSettings: .define("GHOSTTY_ENABLED")
```

## Constants

```swift
// LoopflowCore/LoopflowCore.swift
public let lfdDefaultPort = 2486
public let lfdBaseURL = URL(string: "http://127.0.0.1:2486")!
public let lfdApiBaseURL = lfdBaseURL.appendingPathComponent("v0")
```

---

## Core Models (LoopflowCore/Models/)

### Wave — autonomous AI coding wave

```swift
public struct Wave: Sendable, Identifiable, Hashable {
    public let id: String
    public var name: String
    public var repo: String
    public var flow: String                    // Flow name to run
    public var direction: [String]             // Context directions
    public var area: [String]                  // File paths/globs
    public var stimuli: [Stimulus]             // Triggers (loop, watch, cron)
    public var status: WaveStatus              // idle, running, waiting, failed, paused
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

### WaveStatus

```swift
public enum WaveStatus: String, Sendable, Codable {
    case idle, running, waiting, failed, paused
    public var color: Color { ... }  // statusSuccess/Warning/Neutral/Error
    public var icon: String { ... }  // SF Symbols: circle.fill, xmark.circle.fill, etc.
}
```

### Stimulus — wave trigger

```swift
public struct Stimulus: Sendable, Hashable, Codable, Identifiable {
    public enum Kind: String, Sendable, Codable, CaseIterable { case loop, watch, cron }
    public var id: String
    public var kind: Kind
    public var cron: String?  // For cron stimuli
}
```

### WaveRun — single execution of a wave

```swift
public struct WaveRun: Sendable, Identifiable, Hashable {
    public let id: String
    public let waveId: String?
    public let flow: String
    public let area: String
    public let repo: String
    public let direction: [String]
    public var status: WaveRunStatus     // pending, running, waiting, completed, failed, cancelled
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
    var duration: String?       // "3m45s"
    var relativeTime: String    // "2h ago"
}
```

### WaveViewModel — Wave + enriched git/PR state

Wraps `Wave` API model. Provides derived display properties for the UI.

```swift
public struct WaveViewModel: Sendable, Identifiable, Hashable {
    public var api: Wave                  // Underlying API model
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
    // Execution state
    public var recentSteps: [StepRun]
    public var prLimit: Int              // Default 5
    public var mergeMode: MergeMode      // .pr or .land
    public var pid: Int?
    public var lastMainSha: String?
    public var waitingReason: WaitingReason?
    public var runStartedAt: Date?
    // Forwarded from api
    var id: String { api.id }
    var name: String { get/set → api.name }
    var flow: String { get/set → api.flow }
    var area: [String] { get/set → api.area }
    var direction: [String] { get/set → api.direction }
    var stimuli: [Stimulus] { get/set → api.stimuli }
    var status: WaveStatus { get/set → api.status }
    var iteration: Int { get/set → api.iteration }
    var activeRun: WaveRun? { get/set → api.activeRun }
    // Computed
    var displayName: String       // name or "area · flow"
    var areaDisplay: String
    var statusIndicator: (icon: String, color: Color)
    var pendingPR: (number: Int, url: URL?)?
    var hasOpenPRs: Bool
    var effectiveOpenPRCount: Int
    var lastActivityAt: Date?
    var lastActivityDescription: String?
}
```

### Step + StepRun

```swift
public struct Step: Sendable, Codable, Equatable, Identifiable {
    public var id: UUID
    public var prompt: String
    public var config: StepConfig?  // Optional model, direction, context overrides
}

public struct StepConfig: Sendable, Codable, Equatable {
    public var model: String?
    public var direction: String?
    public var context: [String]?
}

public struct StepRun: Sendable, Identifiable, Codable, Hashable {
    public let id: String
    public let step: String          // Step name
    public let repo: String
    public let worktree: String
    public let status: String        // "running", "waiting", "completed", "error"
    public let startedAt: Date
    public let endedAt: Date?
    public let model: String
    public let runMode: String
}
```

### Flow

```swift
public enum FlowType: String, Sendable, Codable { case flow, step }

public struct Flow: Sendable, Codable, Identifiable, Equatable {
    public var id: UUID
    public var name: String
    public var steps: [Step]
    public var type: FlowType
}
```

### PullRequest

```swift
public enum PRState: String, Sendable, Codable { case open, merged, closed, draft }

public struct PullRequest: Sendable, Hashable, Codable {
    public let url: URL
    public let number: Int?
    public let state: PRState?
    public let title: String?
    public let branch: String?
}
```

### Supporting types

```swift
public enum MergeMode: String, Sendable, Codable { case pr, land }

public enum WaitingReason: Sendable, Hashable {
    case prLimitReached(open: Int, limit: Int)
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
    var command: String  // "lf <step> <prompt>"
}

public enum TerminalApp: String, Sendable, CaseIterable { case warp, iterm, terminal, kitty }
public enum IDEApp: String, Sendable, CaseIterable { case cursor, vscode, zed }
public enum AppearanceMode: String, Sendable, CaseIterable { case system, light, dark }
```

---

## Services (LoopflowCore/Services/)

### WaveServiceProtocol

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
```

### LocalWaveService — HTTP API client for lfd

`public struct LocalWaveService: WaveServiceProtocol, @unchecked Sendable`

Two URLSession configs: fast (3s timeout) and long (30s for git operations).

**Wave CRUD:**
- `listWaves(repo:)` — GET `/v0/waves?repo=<path>&expand[]=active_run`
- `getWave(_:)` — GET `/v0/waves/<id>?expand[]=active_run`
- `createWave(name:repo:)` — POST `/v0/waves` (uses longSession; does git fetch + worktree + push)
- `updateWave(_:config:)` — PATCH `/v0/waves/<id>` (uses longSession; may move worktree)
- `deleteWave(_:)` — DELETE `/v0/waves/<id>`
- `cloneWave(_:name:)` — POST `/v0/waves/<id>/clone`

**Run lifecycle:**
- `run(_:overrides:)` — POST `/v0/waves/<id>/run`
- `stop(_:)` — POST `/v0/waves/<id>/stop`
- `landWave(_:)` — POST `/v0/waves/<id>/land` (creates PR, uses longSession)
- `nextWave(_:)` — POST `/v0/waves/<id>/next` (new iteration/branch)

**Stimulus:**
- `addStimulus(_:kind:cron:)` — POST `/v0/waves/<id>/stimulus`
- `removeStimulus(_:stimulusId:)` — DELETE `/v0/waves/<id>/stimulus/<sid>`

**Advanced:**
- `collapsePRs(_:)` — POST `/v0/waves/<id>/collapse` → `CollapsePRsResult`
- `absorbIntoPR(_:prNumber:)` — POST `/v0/waves/<id>/absorb` → `AbsorbIntoPRResult`
- `connect(_:)` — POST `/v0/waves/<id>/connect` → `ConnectionInfo`
- `streamOutput(waveId:)` — GET `/v0/waves/<id>/logs` → `AsyncThrowingStream<String, Error>`
- `listFlowsAndDirections(repo:)` — GET `/v0/flows?repo=<path>` → `WaveFlowsResult`
- `listWaveRuns(waveId:repo:limit:)` — GET `/v0/wave_runs`

**Supporting types:**
```swift
public struct WaveConfigUpdate: Sendable { name, area, direction, flow, status (all optional) }
public struct RunOverrides: Sendable { area, direction, flow (all optional) }
public struct ConnectionInfo: Sendable { worktree, step, agentId, promptFile, waveRunId, stepIndex }
public struct CollapsePRsResult: Sendable { newPRUrl: String?, closedPRs: [Int] }
public struct AbsorbIntoPRResult: Sendable { targetBranch: String, commitsAbsorbed: Int }
public struct WaveFlowsResult: Sendable { flows: [Flow], directions: [String] }
public enum WaveServiceError: LocalizedError { case commandFailed(String) }
```

### LocalEventService — WebSocket subscription

`public actor LocalEventService`

Connects to `ws://127.0.0.1:2486/ws`. Auto-reconnect with exponential backoff (1s → 5s).

```swift
func subscribe(
    onEvent: @escaping @Sendable (LFDEvent) -> Void,
    onConnectionChange: @escaping @Sendable (Bool) -> Void
) async

func disconnect() async
var isConnected: Bool
```

**Event types:**
```swift
public enum LFDEvent: Sendable {
    case connected(ConnectedEvent)       // Initial wave list on connect
    case wave(WaveEvent)                 // created/updated/deleted/started/stopped/waiting
    case worktree(WorktreeEvent)
    case agentStarted(AgentStartedEvent)
    case agentEnded(AgentEndedEvent)
    case output(OutputEvent)             // Live agent output line
}

public enum WaveEventType: String { case created, updated, deleted, started, stopped, waiting }
public struct WaveEvent { type, waveId, waveRunId?, step?, name?, wave?, timestamp }
public struct OutputEvent { waveId, agentId, text, timestamp }
```

### AuthService — OAuth + Keychain

```swift
public final class AuthService: NSObject, @unchecked Sendable
    func signIn() async throws -> String    // ASWebAuthenticationSession → loopflow://auth/callback
    func signOut() throws
    func currentToken() -> String?
    func tokenExpiresAt() -> Date?          // JWT exp claim
    func refreshToken() async throws -> String  // POST /auth/refresh
    // Keychain: service="studio.loopflow.auth", account="jwt"
```

### AuthState — observable auth wrapper

```swift
@MainActor @Observable
public final class AuthState {
    var token: String?
    var isLoading: Bool
    var error: AuthError?
    var isAuthenticated: Bool  // token != nil && !isExpired
    var needsRefresh: Bool     // expires within 24h
    func signIn() async
    func signOut()
    // Auto-refresh monitor runs hourly
}
```

### AuthError

```swift
public enum AuthError: Error, Sendable {
    case noCallback, invalidCallback, notAuthenticated, tokenExpired
    case sessionFailed, refreshFailed(String)
    case keychainWrite(OSStatus), keychainDelete(OSStatus), unknown(Error)
}
```

### TokenProvider

```swift
public protocol TokenProvider: Sendable { func token() async throws -> String }
public struct NoAuthProvider: TokenProvider { ... }
public final class KeychainTokenProvider: TokenProvider { ... }
```

### NotificationService — macOS notifications

```swift
public final class NotificationService: NSObject, UNUserNotificationCenterDelegate {
    static let shared: NotificationService
    func requestAuthorization() async throws
    func notifyNeedsInteractive(waveId:, waveName:, step:)  // "waiting: <step>"
    func notifyError(waveId:, waveName:, message:)           // "<name> failed"
    func notifyPRReady(waveId:, waveName:, prNumber:)        // "PR #N ready"
    // Taps post Notification.Name.selectWave with waveId
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
    static func logDirectory() -> URL  // ~/Library/Logs/Concerto/
}
```

---

## Concerto State Management (Concerto/State/)

### RepoState — primary app state

`@MainActor @Observable final class RepoState`

```swift
// Data
var currentRepo: URL?
var flows: [Flow]
var availableDirections: [String]
let waveStore: WaveStore
let runStore: RunStore

// Selection
var selectedWaveId: String?
var selectedWave: WaveViewModel?     // derived from store

// Connection
var lfdConnected: Bool
var isLoading: Bool
var errorMessage: String?

// In-flight actions
var inFlightActions: Set<String>     // wave IDs with pending land/etc.

// Lifecycle
func openRepo(_ url: URL, outputBuffer: OutputBuffer, skipBackgroundRefresh: Bool) async
func startEventSubscription(outputBuffer: OutputBuffer)
func refreshWaves() async
func refreshFlowsAsync() async
func loadRuns(for waveId: String)
func connectLfd(outputBuffer:) async throws

// Wave operations (all use optimistic update pattern)
func createWave(name: String) async throws
func runWave(wave:, area:, direction:, flow:) async throws
func stopWave(_ wave: WaveViewModel) async throws
func landWave(_ wave: WaveViewModel) async throws
func nextWave(_ wave: WaveViewModel) async throws
func deleteWave(_ wave: WaveViewModel) async throws
func cloneWave(_ wave: WaveViewModel) async throws -> WaveViewModel
func renameWave(_ wave:, to:) async throws
func updateWave(_ wave:, area:, direction:, flow:, status:) async throws
func addStimulus(wave:, kind:, cron:) async throws
func removeStimulus(wave:, stimulusId:) async throws
func collapsePRs(_ waveId:) async throws -> CollapsePRsResult
func absorbIntoPR(_ waveId:, prNumber:) async throws -> AbsorbIntoPRResult

// UI test / screenshot modes
enum UITestMode: String { case emptyWorkspaces, sampleWorkspaces, mockWaves }
struct ScreenshotMode { outputPath, repoPath?, windowSize?, selectBranch?, mockLoops, mockConfig?, selectTab? }
func configureForUITest(_ mode:, repoURL:)
func configureMockWaves()
```

**Optimistic update pattern:** Apply mutation → run API call → commit on success / rollback on error. Actions (run/stop/next) also schedule a safety-net refresh after 10s.

### WaveStore — dictionary-keyed wave storage

`@MainActor @Observable final class WaveStore`

```swift
// Storage
private(set) var waves: [String: WaveViewModel]  // keyed by wave ID, triggers recompute on set
private(set) var ordered: [WaveViewModel]          // derived
private(set) var groups: WaveGroups                // derived

// Mutations
func set(_ wave: WaveViewModel)           // skipped if pending mutation
func setAll(_ newWaves: [WaveViewModel])  // preserves pending mutations
func remove(_ id: String) -> WaveViewModel?
func removeAll()

// Optimistic
func applyOptimistic(_ id:, _ mutation:) -> WaveViewModel?  // returns snapshot
func commitMutation(_ id:)
func rollback(_ snapshot: WaveViewModel)
func insertPending(_ wave:)
func replacePending(_ pendingId:, with:)
func removePending(_ id:)
func applyDelete(_ id:)

// Queries
func wave(for id: String) -> WaveViewModel?

// Status tracking
var onStatusChange: ((WaveViewModel, WaveStatus?, WaveStatus) -> Void)?
```

### WaveGroups — sidebar categorization

```swift
struct WaveGroups {
    let blocked: [WaveViewModel]        // status == .failed
    let pr: [WaveViewModel]             // hasOpenPRs
    let recentActivity: [WaveViewModel] // activity in last hour (max 5)
    let active: [WaveViewModel]         // running or waiting
    let idle: [WaveViewModel]           // idle, no PRs
    var attentionCount: Int
    var openPRCount: Int
    var allInOrder: [WaveViewModel]     // concatenated in display order
}
```

### OutputBuffer — agent output buffering

`@MainActor @Observable final class OutputBuffer`

```swift
// Output per wave (max 2000 lines)
func appendOutput(waveId:, text:, timestamp:)   // from WebSocket (skipped if stream active)
func output(for waveId:) -> [OutputLine]
func clearOutput(for waveId:)
func recentOutput(for waveId:, maxLength:) -> String?

// HTTP streaming (replay + follow)
func startStreaming(waveId:)    // GET /v0/waves/<id>/logs
func stopStreaming(waveId:)

// Interactive sessions (one at a time)
var interactiveSession: InteractiveSession?
func launchInteractiveSession(waveId:, step:, worktreePath:, prompt:)
func endInteractiveSession()
func hasActiveSession(for waveId:) -> Bool
```

### RunStore — cached wave runs

```swift
@MainActor @Observable final class RunStore {
    private(set) var runs: [String: [WaveRun]]    // waveId → runs (max 50)
    func setRuns(for waveId:, _ newRuns:)
    func runs(for waveId:) -> [WaveRun]
    func clear(for waveId:)
}
```

---

## Design System (Concerto/)

### BrandColors + LoopflowPalette

```swift
// Brand colors (named — used directly in views)
Color.loopflowBurgundy     = #722F37   // Primary accent, .tint()
Color.loopflowBurgundyHover = #8B3D47
Color.loopflowCream         = #FAF8F5   // DarkButtonStyle text

// Status colors (LoopflowCore)
Color.statusSuccess  = #2D6A4F  // Green
Color.statusError    = #B45309  // Rust orange
Color.statusWarning  = #B0812A  // Amber
Color.statusInfo     = #0AB3CC  // Cyan
Color.statusNeutral  = #8B8B8B  // Gray

// Palettes (environment key: \.palette) — hex values inline, no named intermediates
struct LoopflowPalette { background, surface, surfaceMuted, border, text, textSecondary, accent, accentHover }
static let light    // Cream backgrounds, dark text
static let dark     // Slate backgrounds, light text
static let deepWine // #1E1215 → #2A1A20 → #35222A, accent #8B2252
```

### DesignSystem

```swift
enum Spacing     { xxs=2, xs=4, sm=8, md=12, lg=16, xl=20, xxl=24, xxxl=32 }
enum HitTarget   { minimum=24, comfortable=32, touch=44 }
enum ZIndex      { base=0, dropdown=100, modal=200, toast=300, tooltip=400 }
enum CornerRadius { sm=4, md=8, lg=12, xl=16, full=9999 }

enum Typography {
    static let serifFamily = "Cormorant Garamond"  // Headlines
    static let sansFamily  = "Lato"                 // Body, UI
    static let monoFamily  = "JetBrains Mono"       // Code
    func heroTitle(_ size: CGFloat = 32) -> Font
    func sectionTitle(_ size: CGFloat = 20) -> Font
    func body(_ size: CGFloat = 14) -> Font
    func code(_ size: CGFloat = 13) -> Font
}

// View modifiers
.accessibleButton(_ label:, hint:)
.accessibleToggle(_ label:, isOn:)
.minHitTarget()
.keyboardFocusRing(_ isFocused:, cornerRadius:)
```

### Flags

```swift
enum Flags {
    static var beta: Bool          // UserDefaults "beta"
    static func setBeta(_ enabled:)
}
```

---

## Ghostty Integration (Concerto/Services/Ghostty/)

Embedded terminal via libghostty C API. Conditionally compiled with `GHOSTTY_ENABLED`.

```swift
@MainActor final class GhosttyManager: ObservableObject {
    enum State: Equatable { case uninitialized, initializing, ready, failed(String) }
    static let shared: GhosttyManager
    var state: State
    var onSessionClosed: (() -> Void)?

    func initialize()                     // ghostty_init + config + app creation
    func tick()                           // ghostty_app_tick (called from wakeup_cb)
    func createSurface(workingDirectory:, command:, view: NSView) -> ghostty_surface_t?
    func destroySurface(_ surface:)
    func registerActiveSession(_ surface:, sessionId:)
    func destroyActiveSession()
    func sendText(_ text:)
}

// Theme: cream text (#F5E6D3) on burgundy background (#4A1A2C)
// Font: 13pt

enum TerminalStatus: Equatable { case initializing, running, completed(exitCode:), failed(error:) }
struct GhosttySession: Identifiable { id, worktree, command: [String], status, surface? }
```

---

## App Entry Point

```swift
@main struct ConcertoApp: App {
    // Scenes:
    // 1. WindowGroup (default) — WelcomeWindow or ScreenshotWindow or RepoWindow (UI test)
    //    Default size: 500x400
    // 2. WindowGroup(id: "repo", for: URL.self) — RepoWindow per repository
    //    Default size: 900x700
    // 3. Window("Terminal Test", id: "terminal-test") — GhosttyTerminalView
    //    Default size: 800x600
    //
    // Menu commands:
    //   - Beta Features toggle
    //   - Appearance picker (system/light/dark)
    //   - Snapshot for Review (Cmd+4)
    //   - Command Palette (Cmd+K)
    //   - Terminal Test (Cmd+Shift+T)
    //
    // All views tinted .loopflowBurgundy with resolved palette injected via .environment(\.palette)
}
```

---

## Communication Architecture

Two channels to lfd daemon:

1. **HTTP API** (`LocalWaveService`) — reads/writes waves, runs, flows. Port 2486, base `/v0`.
2. **WebSocket** (`LocalEventService`) — live event subscription at `ws://127.0.0.1:2486/ws`. Pushes wave state changes, output lines. Auto-reconnect with backoff.

**Optimistic UI pattern:** `WaveStore` tracks pending mutations. Apply change immediately, commit on API success, rollback on error. WebSocket events skip waves with pending mutations to prevent overwriting optimistic state.

**Output dual-path:** WebSocket pushes `output_line` events → `OutputBuffer.appendOutput()`. When a wave is selected, `OutputBuffer.startStreaming()` connects via HTTP streaming (replay + follow) and suppresses WebSocket output for that wave.

---

## Key Patterns

- **@Observable + @MainActor:** RepoState, WaveStore, OutputBuffer, RunStore all use Swift Observation on main actor
- **Actor isolation:** LocalEventService is an actor for thread-safe WebSocket handling
- **Dictionary-keyed state:** WaveStore uses `[String: WaveViewModel]` with derived `ordered` and `groups` recomputed on every mutation
- **Optimistic mutations:** Snapshot before mutation → API call → commit/rollback. Pending set prevents WebSocket events from overwriting
- **Observation isolation:** Sidebar live output preview has separate `@Environment(OutputBuffer.self)` scope to avoid re-rendering entire sidebar
- **Screenshot mode:** CLI args `--snapshot`, `--repo`, `--size`, `--select`, `--mock-loops`, `--mock-config`, `--tab`
- **Name generation:** `NameGenerator.generate()` returns "magical-musical" pairs (e.g., "aurora-allegro", "crystal-melody")
- **Palette injection:** `LoopflowPalette` resolved from appearance mode, injected via `EnvironmentValues.palette`
- **Conditional compilation:** Ghostty behind `#if GHOSTTY_ENABLED` / `#if canImport(GhosttyKit)`

## Build & Test

```bash
swift build --package-path swift            # Build all targets
swift test --package-path swift             # Run ConcertoTests + SymphoniaTests
./swift/dev run                             # Build and launch Concerto
./swift/dev ui-test                         # Generate Xcode project + run UI tests
./swift/dev release                         # Build .app and .dmg
```
