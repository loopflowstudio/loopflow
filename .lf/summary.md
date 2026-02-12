# Swift Codebase Summary

Package: `LoopflowSwift` — macOS 15+, Swift 6
Products: `LoopflowCore` (library), `Concerto` (app), `Symphonia` (placeholder)
Dependencies: ViewInspector 0.10+, GhosttyKit (binary xcframework)

Build: `./dev run` (SPM), `./dev test`, `./dev xcode` (XcodeGen from `project.yml`)
Install target: `~/Applications/Concerto Dev.app`

---

## File Structure

```
swift/
├── Package.swift
├── project.yml                          # XcodeGen
├── LoopflowCore/                        # Shared library
│   ├── LoopflowCore.swift               # Constants: lfdDefaultPort=2486, lfdBaseURL, lfdApiBaseURL
│   ├── Models/
│   │   ├── Wave.swift                   # Wave, WaveStatus, Stimulus, CommitEntry, InteractiveSession, MergeMode, WaitingReason
│   │   ├── WaveRun.swift                # WaveRun, WaveRunStatus
│   │   ├── WaveViewModel.swift          # WaveViewModel (UI-enriched Wave)
│   │   ├── Flow.swift                   # Flow, FlowType
│   │   ├── Step.swift                   # Step, StepConfig, StepRun
│   │   ├── PullRequest.swift            # PullRequest, PRState
│   │   ├── AppPreferences.swift         # TerminalApp, IDEApp enums
│   │   ├── AppearanceMode.swift         # AppearanceMode (system/light/dark)
│   │   └── StatusColors.swift           # Color extensions: statusSuccess/Error/Warning/Info/Neutral
│   └── Services/
│       ├── LocalWaveService.swift       # HTTP client for lfd API
│       ├── LocalEventService.swift      # WebSocket client for lfd events
│       ├── WaveServiceProtocol.swift    # WaveServiceProtocol, WaveFlowsResult
│       ├── AuthService.swift            # OAuth via ASWebAuthenticationSession, Keychain JWT
│       ├── AuthState.swift              # Observable auth state with auto-refresh
│       ├── AuthError.swift              # AuthError enum
│       ├── LoggingService.swift         # File logger to ~/Library/Logs/Concerto/
│       ├── NotificationService.swift    # UNUserNotificationCenter for wave alerts
│       └── TokenProvider.swift          # TokenProvider protocol, KeychainTokenProvider
├── Concerto/                            # Main macOS app
│   ├── ConcertoApp.swift                # @main, WindowGroup scenes, font registration
│   ├── Flags.swift                      # UserDefaults beta toggle
│   ├── ScriptCommands.swift             # AppleScript: CaptureScreenshotCommand
│   ├── Models/
│   │   └── RecentRepo.swift             # RecentRepo (path, lastOpened)
│   ├── State/
│   │   ├── WaveStore.swift              # @Observable dict-keyed wave storage with optimistic updates
│   │   ├── OutputBuffer.swift           # @Observable output line buffer with HTTP streaming
│   │   └── RunStore.swift               # @Observable cached wave runs per wave
│   ├── Services/
│   │   ├── TerminalLauncher.swift       # Launch Warp/iTerm/Terminal/Kitty, open Cursor/VSCode/Zed
│   │   ├── SetupService.swift           # Check/install lf, uv, claude, lfd daemon
│   │   ├── SnapshotService.swift        # Window capture via bitmapImageRepForCachingDisplay
│   │   ├── RecentsService.swift         # UserDefaults recent repos (max 10)
│   │   ├── RecentAreasService.swift     # UserDefaults recent areas per repo (max 5)
│   │   ├── NameGenerator.swift          # Random wave names: magical + musical words
│   │   ├── AppIconProvider.swift        # NSWorkspace app icon lookup
│   │   └── Ghostty/
│   │       ├── GhosttyManager.swift     # Singleton wrapping ghostty_app_t C API
│   │       └── GhosttyTypes.swift       # TerminalStatus, GhosttySession
│   ├── BrandColors.swift                # LoopflowPalette (light/dark/deepWine), Color.loopflowBurgundy
│   ├── DesignSystem.swift               # Spacing, HitTarget, ZIndex, CornerRadius, Typography, DesignAnimation
│   ├── Views/
│   │   ├── ContentView.swift            # Main split view (sidebar + detail), command palette overlay
│   │   ├── WelcomeWindow.swift          # Launch screen with recent repos, "Open Folder"
│   │   ├── RepoWindow.swift             # Per-repo window: setup check → ContentView
│   │   ├── ScreenshotWindow.swift       # Automated screenshot mode window
│   │   ├── WaveSidebar.swift            # Sidebar: grouped wave list with keyboard nav
│   │   ├── WaveRow.swift                # Single wave row: name, status, PR badge, activity
│   │   ├── WaveDetailPanel.swift        # Detail: tabs (Current/Runs), header, actions
│   │   ├── WaveRunsTab.swift            # Historical runs list with Collapse/Absorb actions
│   │   ├── StepRunner.swift             # Idle-state step execution UI
│   │   ├── FlowProgressPills.swift      # Current step pills with elapsed time
│   │   ├── NextActionsBar.swift         # Post-completion action buttons
│   │   ├── WaitingStateCard.swift       # PR limit reached card
│   │   ├── LiveOutput.swift             # Terminal-style output display
│   │   ├── InteractiveSessionView.swift # Embedded terminal for interactive steps
│   │   ├── EmbeddedTerminalPanel.swift  # Ghostty terminal panel wrapper
│   │   ├── CommandPalette.swift         # Cmd+K palette: fuzzy search, keyboard nav
│   │   ├── AreaTypeahead.swift          # Path input with tab completion, multi-select chips
│   │   ├── FlowTypeahead.swift          # Flow/step picker with ghost completion
│   │   ├── DirectionTypeahead.swift     # Direction picker with ghost completion
│   │   ├── TypeaheadComponents.swift    # GhostTextField, TypeaheadChip, WrappingHStack
│   │   ├── IterationTimeline.swift      # Visual timeline of wave runs
│   │   ├── QuickExperimentView.swift    # Quick experiment launcher
│   │   ├── SetupView.swift              # First-run install flow
│   │   ├── DiagnosticsView.swift        # Debug/diagnostics panel
│   │   ├── ThemePreview.swift           # Design system preview
│   │   └── TerminalTestWindow.swift     # Ghostty testing window
│   └── Fonts/                           # Bundled: CormorantGaramond, Lato, JetBrainsMono
├── ConcertoTests/                       # Unit tests
├── ConcertoUITests/                     # Screenshot pipeline tests
├── Symphonia/                           # Placeholder second app
└── SymphoniaTests/
```

---

## Core Data Structures

### Wave — autonomous AI coding wave

```swift
// LoopflowCore/Models/Wave.swift
public struct Wave: Sendable, Identifiable, Hashable {
    public let id: String
    public var name: String
    public var repo: String                // Absolute path to git repo
    public var flow: String                // Flow/step name to execute
    public var direction: [String]         // Prompt modifier filenames
    public var area: [String]              // Code areas to focus on
    public var stimuli: [Stimulus]         // Automation triggers
    public var status: WaveStatus          // idle/running/waiting/failed/paused
    public var iteration: Int              // Current iteration number
    public var localWorktree: String?      // Absolute path to git worktree
    public var remoteBranch: String?       // Remote branch name
    public var commits: [CommitEntry]      // Commits on this wave's branch
    public var diffStat: String?           // Git diff stat summary
    public var flowSteps: [String]         // Step names in the flow
    public var openPRCount: Int            // Number of open PRs for this wave
    public var activeRun: WaveRun?         // Currently executing run (expanded)
    public var createdAt: Date?
}
```

### WaveStatus

```swift
public enum WaveStatus: String, Sendable, Codable {
    case idle, running, waiting, failed, paused
}
```

### Stimulus — automation trigger

```swift
public struct Stimulus: Sendable, Hashable, Codable, Identifiable {
    public enum Kind: String, Sendable, Codable, CaseIterable {
        case loop    // Continuous re-run
        case watch   // On file change
        case cron    // Scheduled (cron expression)
    }
    public var id: String
    public var kind: Kind
    public var cron: String?   // Cron expression, only for .cron kind
}
```

### WaveRun — single execution of a wave

```swift
// LoopflowCore/Models/WaveRun.swift
public struct WaveRun: Sendable, Identifiable, Hashable {
    public let id: String
    public let waveId: String?
    public let flow: String
    public let area: String            // Joined area string
    public let repo: String
    public let direction: [String]
    public var status: WaveRunStatus   // pending/running/waiting/completed/failed/cancelled
    public var iteration: Int
    public var stepIndex: Int          // Current step within flow
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

### WaveViewModel — UI-enriched wave for display

```swift
// LoopflowCore/Models/WaveViewModel.swift
public struct WaveViewModel: Sendable, Identifiable, Hashable {
    public var api: Wave                  // Core wave data from API
    public var worktreePath: String?      // Resolved from api.activeRun or api.localWorktree
    public var branch: String?
    public var isDirty: Bool              // Git working tree dirty
    public var isRebasing: Bool
    public var isMerging: Bool
    public var hasDiff: Bool              // Has uncommitted changes
    public var aheadMain: Int             // Commits ahead of main
    public var behindMain: Int
    public var aheadRemote: Int           // Commits ahead of remote tracking
    public var behindRemote: Int
    public var prURL: URL?
    public var prNumber: Int?
    public var prState: PRState?          // open/merged/closed/draft
    public var recentSteps: [StepRun]     // Recent step executions
    public var prLimit: Int               // Max concurrent PRs (default 5)
    public var mergeMode: MergeMode       // .pr or .land
    public var pid: Int?                  // Agent process ID
    public var lastMainSha: String?
    public var waitingReason: WaitingReason?
    public var runStartedAt: Date?

    // Computed: id, name, repo, flow, direction, area, stimuli, status,
    //           iteration, activeRun, createdAt (all delegate to api)
    // displayName, areaDisplay, directionDisplay, statusText, detailText
    // statusIndicator -> (icon: String, color: Color)
    // hasOpenPRs, effectiveOpenPRCount, pendingPR, lastActivityAt
}
```

### Flow & Step

```swift
// LoopflowCore/Models/Flow.swift
public enum FlowType: String, Sendable, Codable { case flow, step }

public struct Flow: Sendable, Codable, Identifiable, Equatable {
    public var id: UUID
    public var name: String
    public var steps: [Step]
    public var type: FlowType   // .flow (multi-step) or .step (single)
}

// LoopflowCore/Models/Step.swift
public struct Step: Sendable, Codable, Equatable, Identifiable {
    public var id: UUID
    public var prompt: String       // Step/prompt name
    public var config: StepConfig?  // Optional overrides
}

public struct StepConfig: Sendable, Codable, Equatable {
    public var model: String?       // LLM model override
    public var direction: String?   // Direction override
    public var context: [String]?   // Additional context files
}

public struct StepRun: Sendable, Identifiable, Codable, Hashable {
    public let id: String
    public let step: String         // Step name
    public let repo: String
    public let worktree: String
    public let status: String       // "running"/"waiting"/"completed"/"error"
    public let startedAt: Date
    public let endedAt: Date?
    public let model: String
    public let runMode: String
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

### Supporting Types

```swift
public struct CommitEntry: Sendable, Hashable, Identifiable {
    public let sha: String
    public let message: String
}

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
    var command: String { "lf \(step)" }  // Shell command to run
}

public enum TerminalApp: String, Sendable, CaseIterable { case warp, iterm, terminal, kitty }
public enum IDEApp: String, Sendable, CaseIterable { case cursor, vscode, zed }
public enum AppearanceMode: String, Sendable, CaseIterable { case system, light, dark }
```

---

## Communication with lfd Daemon

Two protocols, intentionally separate:

### HTTP API — `LocalWaveService`

Base: `http://127.0.0.1:2486/v0`
Timeouts: 3s request / 10s resource (default), 30s/60s for git ops (longSession)

```swift
public struct LocalWaveService: WaveServiceProtocol, @unchecked Sendable {
    // MARK: - Wave CRUD
    func listWaves(repo: URL) async throws -> [Wave]          // GET /v0/waves?repo=...&expand[]=active_run
    func getWave(_ id: String) async throws -> Wave            // GET /v0/waves/{id}?expand[]=active_run
    func createWave(name: String, repo: URL) async throws -> Wave  // POST /v0/waves
    func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave  // PATCH /v0/waves/{id}
    func deleteWave(_ id: String) async throws                 // DELETE /v0/waves/{id}
    func cloneWave(_ id: String, name: String?) async throws -> Wave  // POST /v0/waves/{id}/clone

    // MARK: - Wave Actions
    func run(_ id: String, overrides: RunOverrides?) async throws     // POST /v0/waves/{id}/run
    func stop(_ id: String) async throws                              // POST /v0/waves/{id}/stop
    func landWave(_ id: String) async throws                          // POST /v0/waves/{id}/land
    func nextWave(_ id: String) async throws -> String                // POST /v0/waves/{id}/next
    func connect(_ id: String) async throws -> ConnectionInfo         // POST /v0/waves/{id}/connect
    func collapsePRs(_ id: String) async throws -> CollapsePRsResult  // POST /v0/waves/{id}/collapse
    func absorbIntoPR(_ id: String, prNumber: Int) async throws -> AbsorbIntoPRResult  // POST /v0/waves/{id}/absorb

    // MARK: - Stimulus
    func addStimulus(_ waveId: String, kind: Stimulus.Kind, cron: String?) async throws -> Stimulus  // POST /v0/waves/{id}/stimulus
    func removeStimulus(_ waveId: String, stimulusId: String) async throws  // DELETE /v0/waves/{id}/stimulus/{sid}

    // MARK: - Queries
    func listFlowsAndDirections(repo: URL) async throws -> WaveFlowsResult  // GET /v0/flows?repo=...
    func listWaveRuns(waveId: String?, repo: URL?, limit: Int) async throws -> [WaveRun]  // GET /v0/wave_runs
    func streamOutput(waveId: String) -> AsyncThrowingStream<String, Error>  // GET /v0/waves/{id}/logs (line streaming)

    // MARK: - Daemon
    func connectLfd() async throws         // Runs `lfd install`
    func checkAvailability() async -> Bool  // GET /status
}
```

#### Request/Response Types

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

public struct WaveFlowsResult: Sendable {
    public var flows: [Flow]
    public var directions: [String]
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

### WebSocket — `LocalEventService`

URL: `ws://127.0.0.1:2486/ws`
Auto-reconnect: 1s for first 10 attempts, then 5s

```swift
public actor LocalEventService {
    func subscribe(
        onEvent: @escaping @Sendable (LFDEvent) -> Void,
        onConnectionChange: @escaping @Sendable (Bool) -> Void
    ) async
    func disconnect() async
    var isConnected: Bool { get }
}

public enum LFDEvent: Sendable {
    case connected(ConnectedEvent)         // Initial handshake, includes all waves
    case wave(WaveEvent)                   // wave_created/updated/deleted/started/stopped/waiting
    case worktree(WorktreeEvent)           // worktree_updated
    case agentStarted(AgentStartedEvent)   // agent_started
    case agentEnded(AgentEndedEvent)       // agent_ended
    case output(OutputEvent)               // output_line (live agent output)
}

public struct ConnectedEvent: Sendable {
    public let timestamp: Date
    public let waves: [Wave]               // Full wave list on connect
}

public struct WaveEvent: Sendable {
    public let type: WaveEventType         // created/updated/deleted/started/stopped/waiting
    public let waveId: String
    public let waveRunId: String?
    public let step: String?
    public let name: String?
    public let wave: Wave?                 // Full wave data (on created/updated)
    public let timestamp: Date
}

public enum WaveEventType: String, Sendable {
    case created, updated, deleted, started, stopped, waiting
}

public struct WorktreeEvent: Sendable { let worktree: String; let repo: String; let branch: String?; let timestamp: Date }
public struct AgentStartedEvent: Sendable { let agentId: String; let step: String; let worktree: String; let timestamp: Date }
public struct AgentEndedEvent: Sendable { let agentId: String; let status: String; let timestamp: Date }
public struct OutputEvent: Sendable { let waveId: String; let agentId: String; let text: String; let timestamp: Date }
```

---

## App State Layer (Concerto)

### WaveStore — dictionary-keyed observable wave storage

```swift
// Concerto/State/WaveStore.swift
@MainActor @Observable
final class WaveStore {
    private(set) var waves: [String: WaveViewModel]  // Primary storage, keyed by wave ID
    private(set) var ordered: [WaveViewModel]         // Sorted: blocked + pr + recentActivity + active + idle
    private(set) var groups: WaveGroups               // Categorized subsets

    // Optimistic update pattern:
    func applyOptimistic(_ id: String, _ mutation: (inout WaveViewModel) -> Void) -> WaveViewModel?
    func commitMutation(_ id: String)
    func rollback(_ snapshot: WaveViewModel)

    // CRUD:
    func set(_ wave: WaveViewModel)           // Set single wave (skipped if pending mutation)
    func setAll(_ newWaves: [WaveViewModel])  // Replace all (preserves pending mutations)
    func remove(_ id: String) -> WaveViewModel?

    // Pending create/delete:
    func insertPending(_ wave: WaveViewModel)
    func replacePending(_ pendingId: String, with wave: WaveViewModel)
    func applyDelete(_ id: String)

    // Notification callback:
    var onStatusChange: ((WaveViewModel, WaveStatus?, WaveStatus) -> Void)?
}

struct WaveGroups {
    let blocked: [WaveViewModel]        // status == .failed
    let pr: [WaveViewModel]             // hasOpenPRs
    let recentActivity: [WaveViewModel] // Activity within last hour (max 5)
    let active: [WaveViewModel]         // running/waiting, not in recentActivity
    let idle: [WaveViewModel]           // idle, not in recentActivity
    var attentionCount: Int             // blocked.count + pr.count
    var openPRCount: Int                // Sum of effectiveOpenPRCount across pr group
}
```

### OutputBuffer — agent output streaming

```swift
// Concerto/State/OutputBuffer.swift
@MainActor @Observable
final class OutputBuffer {
    private var waveOutput: [String: [OutputLine]]   // Keyed by wave ID, max 2000 lines
    private var streamTasks: [String: Task<Void, Never>]

    var interactiveSession: InteractiveSession?

    func appendOutput(waveId: String, text: String, timestamp: Date)  // From WebSocket (skipped if streaming)
    func output(for waveId: String) -> [OutputLine]
    func clearOutput(for waveId: String)
    func startStreaming(waveId: String)    // HTTP replay+follow via /v0/waves/{id}/logs
    func stopStreaming(waveId: String)
    func recentOutput(for waveId: String, maxLength: Int) -> String?

    func launchInteractiveSession(waveId: String, step: String, worktreePath: String, prompt: String?)
    func endInteractiveSession()
    func hasActiveSession(for waveId: String) -> Bool
}

struct OutputLine: Identifiable {
    let id: UUID
    let text: String
    let timestamp: Date
}
```

### RepoState — primary app state coordinator

```swift
// Concerto/State/RepoState.swift
@MainActor @Observable
final class RepoState {
    var currentRepo: URL?
    var flows: [Flow]
    var availableDirections: [String]
    let waveStore = WaveStore()
    let runStore = RunStore()
    var selectedWaveId: String?
    var selectedWave: WaveViewModel?    // Derived from waveStore + selectedWaveId
    var isLoading: Bool
    var errorMessage: String?
    var lfdConnected: Bool

    // Repo lifecycle
    func openRepo(_ url: URL, outputBuffer: OutputBuffer) async
    func startEventSubscription(outputBuffer: OutputBuffer)
    func refreshWaves() async
    func refreshFlowsAsync() async

    // Wave CRUD (all use optimistic updates with rollback)
    func createWave(name: String) async throws
    func runWave(wave: WaveViewModel, area: [String]?, direction: [String]?, flow: String?) async throws
    func stopWave(_ wave: WaveViewModel) async throws
    func deleteWave(_ wave: WaveViewModel) async throws
    func cloneWave(_ wave: WaveViewModel) async throws -> WaveViewModel
    func renameWave(_ wave: WaveViewModel, to newName: String) async throws
    func updateWave(_ wave: WaveViewModel, area: [String]?, direction: [String]?, flow: String?, status: WaveStatus?) async throws
    func landWave(_ wave: WaveViewModel) async throws
    func nextWave(_ wave: WaveViewModel) async throws
    func addStimulus(wave: WaveViewModel, kind: Stimulus.Kind, cron: String?) async throws
    func removeStimulus(wave: WaveViewModel, stimulusId: String) async throws
    func collapsePRs(_ waveId: String) async throws -> CollapsePRsResult
    func absorbIntoPR(_ waveId: String, prNumber: Int) async throws -> AbsorbIntoPRResult
    func loadRuns(for waveId: String)
    func connectLfd(outputBuffer: OutputBuffer) async throws

    // UI test modes
    enum UITestMode: String { case emptyWorkspaces, sampleWorkspaces, mockWaves }
    struct ScreenshotMode { let outputPath: String; let repoPath: String?; let windowSize: (Int, Int)?; ... }
    func configureForUITest(_ mode: UITestMode, repoURL: URL)
    func configureMockWaves()
}
```

### RunStore — cached wave run history

```swift
// Concerto/State/RunStore.swift
@MainActor @Observable
final class RunStore {
    private(set) var runs: [String: [WaveRun]]   // Keyed by wave ID, max 50 per wave
    func setRuns(for waveId: String, _ newRuns: [WaveRun])
    func runs(for waveId: String) -> [WaveRun]
    func clear(for waveId: String)
}
```

---

## App Entry Point

```swift
// Concerto/ConcertoApp.swift
@main struct ConcertoApp: App {
    // Registers bundled fonts: CormorantGaramond (Regular/Medium/SemiBold),
    //   Lato (Regular/Bold), JetBrainsMono (Regular)
    // Requests notification authorization on launch

    var body: some Scene {
        WindowGroup { ... }                      // Welcome window (default 500x400)
        WindowGroup(id: "repo", for: URL.self)   // Per-repo window (default 900x700)
        Window("Terminal Test", id: "terminal-test")  // Ghostty test (800x600)
    }

    // Menu commands: Beta Features toggle, Appearance picker,
    //   Snapshot for Review (Cmd+4), Command Palette (Cmd+K),
    //   Terminal Test (Cmd+Shift+T)
}
```

### RepoWindow — per-repository window

```swift
// Concerto/Views/RepoWindow.swift
struct RepoWindow: View {
    let repoURL: URL?
    let recentsService: RecentsService
    @State private var repoState = RepoState()
    @State private var outputBuffer = OutputBuffer()
    // Flow: check setup -> show SetupView or ContentView
    // Passes repoState and outputBuffer into environment
}
```

---

## Services

### AuthService — OAuth + JWT Keychain

```swift
// LoopflowCore/Services/AuthService.swift
public final class AuthService: NSObject, @unchecked Sendable {
    // OAuth via ASWebAuthenticationSession
    // Callback scheme: "loopflow://auth/callback"
    // Login URL: https://loopflow.studio/auth/login
    // Refresh URL: https://loopflow.studio/auth/refresh
    // JWT stored in Keychain (service: "studio.loopflow.auth", account: "jwt")

    func signIn() async throws -> String       // Returns JWT
    func signOut() throws
    func currentToken() -> String?
    func tokenExpiresAt() -> Date?
    func refreshToken() async throws -> String
}

// AuthState wraps AuthService with @Observable
@MainActor @Observable
public final class AuthState {
    public private(set) var token: String?
    public private(set) var isLoading: Bool
    public private(set) var error: AuthError?
    public var isAuthenticated: Bool   // token != nil && !isExpired
    public var needsRefresh: Bool      // Expiry within 24h
    // Auto-refreshes every hour in background
}
```

### TerminalLauncher

```swift
// Concerto/Services/TerminalLauncher.swift
struct TerminalLauncher {
    func launchTerminal(_ terminal: TerminalApp, at path: URL, command: String?)  // Warp/iTerm via AppleScript, Kitty via Process
    func openInIDE(_ ide: IDEApp, at path: URL, workspace: String?)              // cursor/code/zed CLI
    func openInFinder(at path: URL)
    func openURL(_ url: URL)
    func launchStep(_ step: String, terminal: TerminalApp, at repo: URL)         // Runs "lf <step>" in terminal
}
```

### SetupService — dependency installation

```swift
// Concerto/Services/SetupService.swift
struct SetupService {
    func checkDependencies() -> DependencyStatus   // Checks for `lf` in standard bin dirs
    func install() async throws                    // Installs uv, then `uv tool install loopflow`, then Node/Claude Code
    func ensureDaemonRunning() async throws        // Checks LaunchAgent plist, runs `lfd install` or `launchctl load`

    struct DependencyStatus { var lfInstalled: Bool; var lfPath: String? }
    // Searches: ~/.local/bin, /opt/homebrew/bin, /usr/local/bin, /usr/bin, ~/.cargo/bin
    // Log file: ~/.lf/logs/concerto-setup.log
}
```

### GhosttyManager — embedded terminal

```swift
// Concerto/Services/Ghostty/GhosttyManager.swift
#if GHOSTTY_ENABLED
@MainActor final class GhosttyManager: ObservableObject {
    static let shared: GhosttyManager
    @Published private(set) var state: State   // uninitialized/initializing/ready/failed

    func initialize()                          // ghostty_init + ghostty_config_new + ghostty_app_new
    func tick()                                // ghostty_app_tick (called from wakeup_cb)
    func createSurface(workingDirectory: String, command: String?, view: NSView) -> ghostty_surface_t?
    func destroySurface(_ surface: ghostty_surface_t)
    func registerActiveSession(_ surface: ghostty_surface_t, sessionId: String)
    func destroyActiveSession()
    func sendText(_ text: String)              // ghostty_surface_text
    var onSessionClosed: (() -> Void)?
}
#endif

// Linked frameworks: Carbon, QuartzCore, Metal, IOKit, libc++
// Custom Loopflow theme: burgundy background (#4A1A2C), cream foreground (#F5E6D3)
```

### Other Services

```swift
// LoggingService — file logging to ~/Library/Logs/Concerto/{category}.log
public enum LoggingService {
    enum Category: String { case worktrees, lfd, general, ui, model }
    static func append(_ message: String, category: Category)
    static func ui(_ message: String)     // User interactions
    static func model(_ message: String)  // Data model changes
    static func lfd(_ message: String)    // lfd communication
}

// NotificationService — macOS user notifications
public final class NotificationService {
    static let shared: NotificationService
    func notifyNeedsInteractive(waveId: String, waveName: String, step: String)
    func notifyError(waveId: String, waveName: String, message: String)
    func notifyPRReady(waveId: String, waveName: String, prNumber: Int)
}

// RecentsService — recent repos (UserDefaults, max 10)
@Observable final class RecentsService {
    private(set) var recentRepos: [RecentRepo]
    func addRecent(_ url: URL)
    func removeRecent(_ url: URL)
}

// RecentAreasService — recent area selections per repo (UserDefaults, max 5)
struct RecentAreasService {
    func recentAreas(for repoURL: URL) -> [String]
    func addRecentArea(_ area: String, for repoURL: URL)
}

// SnapshotService — window capture without Screen Recording permission
@MainActor struct SnapshotService {
    func snapshotKeyWindow() throws -> URL              // Saves to /tmp/concerto-<timestamp>.png
    func snapshotKeyWindow(to outputPath: String) throws -> URL
    func snapshotWindow(_ window: NSWindow, to outputURL: URL) throws -> URL
    // Uses NSView.bitmapImageRepForCachingDisplay (no screencapture)
}

// NameGenerator — random wave names
enum NameGenerator {
    static func generate() -> String  // e.g. "aurora-allegro", "frost-melody"
    // magical: aurora, cascade, crystal, drift, echo, ember, ...
    // musical: allegro, aria, ballad, cadence, canon, chord, ...
}

// AppIconProvider — look up installed app icons
struct AppIconProvider {
    static func icon(for app: AppIdentifier) -> NSImage?
    static func iconImage(for app: AppIdentifier, size: CGFloat) -> Image
}

enum AppIdentifier {
    case cursor, warp, vscode, iterm, terminal, zed, kitty, github
    var bundleIdentifier: String?  // e.g. "dev.warp.Warp-Stable"
}
```

---

## Design System

Defined in DESIGN.md and implemented in view code.

**Spacing** (4pt base): xxs=2, xs=4, sm=8, md=12, lg=16, xl=20, xxl=24, xxxl=32
**Hit Targets**: minimum=24, comfortable=32, touch=44
**Corner Radius**: sm=4, md=8, lg=12, xl=16, full=9999
**Z-Index**: base=0, dropdown=100, modal=200, toast=300, tooltip=400

**Typography** (bundled fonts):
- Serif: Cormorant Garamond (headlines, editorial)
- Sans: Lato (body, UI)
- Mono: JetBrains Mono (code)

**Brand Colors**: `Color.loopflowBurgundy` (#722F37), `.loopflowBurgundyHover` (#8B3D47), `.loopflowCream` (#FAF8F5)
**Status Colors**: `.statusSuccess` (#2D6A4F), `.statusError` (#B45309), `.statusWarning` (#B0812A), `.statusInfo` (#0AB3CC), `.statusNeutral` (#8B8B8B)
**Animations**: `DesignAnimation.standard/fast/spring(reduceMotion)` — respects accessibilityReduceMotion

**LoopflowPalette** (injected via `@Environment(\.palette)`):

```swift
struct LoopflowPalette {
    let background, surface, surfaceMuted, border: Color
    let text, textSecondary, accent, accentHover: Color
    static let light: LoopflowPalette   // Cream/white tones
    static let dark: LoopflowPalette    // Blue-gray tones
    static let deepWine: LoopflowPalette // Wine-red tones
}
```

**Keyboard shortcuts**: Cmd+K (command palette), Cmd+L (focus prompt), Cmd+N (new workspace), Cmd+4 (snapshot), arrows/Enter (sidebar nav), T/I/D/P/L/Delete (wave actions)

---

## Key Patterns

1. **Observable state** — Swift `@Observable` macro (not Combine), `@MainActor` isolation
2. **Optimistic updates** — WaveStore tracks pending mutations, rolls back on failure
3. **Dictionary-keyed storage** — Waves stored as `[String: WaveViewModel]` for O(1) lookup
4. **Derived groups** — `WaveGroups` recomputed on every wave dict mutation (blocked/pr/recentActivity/active/idle)
5. **Dual communication** — HTTP for CRUD/actions, WebSocket for real-time events
6. **Output dedup** — OutputBuffer skips WebSocket output when HTTP streaming is active for a wave
7. **Environment injection** — `RepoState` and `OutputBuffer` passed via SwiftUI `.environment()`
8. **Per-repo windows** — Each `WindowGroup(id: "repo")` gets its own state instances
9. **JSON parsing** — Manual `JSONSerialization` (not Codable) for API responses, handles flexible server schemas
10. **Graceful degradation** — Network errors return empty arrays, never crash; errors thrown only for user-initiated actions
