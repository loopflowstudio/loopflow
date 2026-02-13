# Swift Codebase Summary

## Package Structure

`swift/Package.swift` — SPM package, macOS 15+, three targets:

| Target | Type | Purpose |
|--------|------|---------|
| **LoopflowCore** | Library | Shared models + services |
| **Concerto** | Executable | Main macOS app (wave management) |
| **Symphonia** | Executable | Placeholder / future app |

**Dependencies:** ViewInspector 0.10.0+ (testing), GhosttyKit (binary framework, embedded terminal)

---

## LoopflowCore — Models

### Wave (`LoopflowCore/Models/Wave.swift`)

```swift
public struct Wave: Sendable, Identifiable, Hashable {
    public let id: String
    public var name: String
    public var repo: String              // repository path
    public var flow: String              // flow name to run
    public var direction: [String]       // AI direction/persona
    public var area: [String]            // file scope
    public var stimuli: [Stimulus]       // triggers (loop/watch/cron)
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

public enum WaveStatus: String, Sendable, Codable {
    case idle, running, waiting, failed, paused
}
```

### Stimulus (`LoopflowCore/Models/Wave.swift`)

```swift
public struct Stimulus: Sendable, Hashable, Codable, Identifiable {
    public enum Kind: String, Sendable, Codable, CaseIterable {
        case loop    // continuous
        case watch   // file-system trigger
        case cron    // scheduled
    }
    public var id: String
    public var kind: Kind
    public var enabled: Bool
    public var cron: String?
}
```

### WaveRun (`LoopflowCore/Models/WaveRun.swift`)

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
}

public enum WaveRunStatus: String, Sendable, Codable {
    case pending, running, waiting, completed, failed, cancelled
}
```

### WaveViewModel (`LoopflowCore/Models/WaveViewModel.swift`)

Client-side enriched model combining API data with git/PR state:

```swift
public struct WaveViewModel: Sendable, Identifiable, Hashable {
    public var api: Wave                  // core API data
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
    public var prState: PRState?          // .open | .merged | .closed | .draft
    public var recentSteps: [StepRun]
    public var prLimit: Int
    public var mergeMode: MergeMode
    public var pid: Int?
    public var lastMainSha: String?
    public var waitingReason: WaitingReason?
    public var runStartedAt: Date?

    // Computed: displayName, areaDisplay, directionDisplay, statusText, detailText, etc.
}
```

### Flow & Step (`LoopflowCore/Models/Flow.swift`, `Step.swift`)

```swift
public struct Flow: Sendable, Codable, Identifiable, Equatable {
    public var id: UUID
    public var name: String
    public var steps: [Step]
    public var type: FlowType            // .flow or .step
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
    public let status: String
    public let startedAt: Date
    public let endedAt: Date?
    public let model: String
    public let runMode: String
}
```

### PullRequest (`LoopflowCore/Models/PullRequest.swift`)

```swift
public struct PullRequest: Sendable, Hashable, Codable {
    public let url: URL
    public let number: Int?
    public let state: PRState?
    public let title: String?
    public let branch: String?
}

public enum PRState: String, Sendable, Codable {
    case open, merged, closed, draft
}
```

### Preferences (`LoopflowCore/Models/AppPreferences.swift`, `AppearanceMode.swift`)

```swift
public enum TerminalApp: String, Sendable, CaseIterable { case warp, iterm, terminal, kitty }
public enum IDEApp: String, Sendable, CaseIterable { case cursor, vscode, zed }
public enum AppearanceMode: String, Sendable, CaseIterable { case system, light, dark }
```

### Status Colors (`LoopflowCore/Models/StatusColors.swift`)

```swift
extension Color {
    public static let statusSuccess = Color(hex: 0x2D6A4F)   // green
    public static let statusError   = Color(hex: 0xB45309)   // orange/brown
    public static let statusWarning = Color(hex: 0xB0812A)   // amber
    public static let statusInfo    = Color(hex: 0x0AB3CC)   // cyan
    public static let statusNeutral = Color(hex: 0x8B8B8B)   // gray
}
```

---

## LoopflowCore — Services

### LocalWaveService (`LoopflowCore/Services/LocalWaveService.swift`)

HTTP client to the `lfd` daemon at `http://127.0.0.1:2486/v0`.

```swift
public struct LocalWaveService: WaveServiceProtocol {
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
    func combinePRs(_ id: String) async throws -> CombinePRsResult

    // Stimuli
    func addStimulus(_ waveId: String, kind: Stimulus.Kind, cron: String?) async throws -> Stimulus
    func removeStimulus(_ waveId: String, stimulusId: String) async throws

    // Terminal / IDE
    func connect(_ id: String) async throws -> ConnectionInfo

    // Discovery
    func listFlowsAndDirections(repo: URL) async throws -> WaveFlowsResult
    func listWaveRuns(waveId: String?, repo: URL?, limit: Int) async throws -> [WaveRun]

    // Live
    func streamOutput(waveId: String) -> AsyncThrowingStream<String, Error>
    func checkAvailability() async -> Bool
    func connectLfd() async throws
}
```

**Config constants** (`LoopflowCore/LoopflowCore.swift`):
```swift
public let lfdDefaultPort = 2486
public let lfdBaseURL = URL(string: "http://127.0.0.1:2486")!
public let lfdApiBaseURL = lfdBaseURL.appendingPathComponent("v0")
```

### LocalEventService (`LoopflowCore/Services/LocalEventService.swift`)

WebSocket client for real-time events from `lfd`.

```swift
public actor LocalEventService {
    func subscribe(
        onEvent: @escaping @Sendable (LFDEvent) -> Void,
        onConnectionChange: @escaping @Sendable (Bool) -> Void
    ) async
    func disconnect() async
    var isConnected: Bool
}

public enum LFDEvent: Sendable {
    case connected(ConnectedEvent)
    case wave(WaveEvent)
    case worktree(WorktreeEvent)
    case agentStarted(AgentStartedEvent)
    case agentEnded(AgentEndedEvent)
    case output(OutputEvent)
}
```

### AuthService & AuthState (`LoopflowCore/Services/AuthService.swift`, `AuthState.swift`)

OAuth via loopflow.studio, token in Keychain.

```swift
public final class AuthService {
    func signIn() async throws -> String
    func signOut() throws
    func currentToken() -> String?
    func tokenExpiresAt() -> Date?
    func refreshToken() async throws -> String
}

@MainActor @Observable
public final class AuthState {
    private(set) var token: String?
    private(set) var isLoading: Bool
    private(set) var error: AuthError?
    var isAuthenticated: Bool
    var isExpired: Bool
    func signIn() async
    func signOut()
}
```

### LoggingService (`LoopflowCore/Services/LoggingService.swift`)

```swift
public enum LoggingService {
    public enum Category: String { case worktrees, lfd, general, ui, model }
    static func append(_ message: String, category: Category)
    static func ui(_ message: String)
    static func model(_ message: String)
    static func lfd(_ message: String)
    static func read(category: Category) -> String
    static func logPath(category: Category) -> String
}
```

### NotificationService (`LoopflowCore/Services/NotificationService.swift`)

```swift
public final class NotificationService: UNUserNotificationCenterDelegate {
    static let shared: NotificationService
    func requestAuthorization() async throws
    func notifyNeedsInteractive(waveId: String, waveName: String, step: String)
    func notifyError(waveId: String, waveName: String, message: String)
    func notifyPRReady(waveId: String, waveName: String, prNumber: Int)
}
```

---

## Concerto App

### Design System (`Concerto/BrandColors.swift`, `Concerto/DesignSystem.swift`)

```swift
// Brand
static let loopflowBurgundy     = Color(hex: 0x722F37)
static let loopflowBurgundyHover = Color(hex: 0x8B3D47)
static let loopflowCream        = Color(hex: 0xFAF8F5)

struct LoopflowPalette {
    let background, surface, surfaceMuted, border: Color
    let text, textSecondary, accent, accentHover: Color
    static let light, dark, deepWine: LoopflowPalette
}

// 4pt spacing scale
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
    static let sansFamily  = "Lato"                 // Body/UI
    static let monoFamily  = "JetBrains Mono"       // Code
    static func heroTitle(_ size: CGFloat = 32) -> Font
    static func sectionTitle(_ size: CGFloat = 20) -> Font
    static func body(_ size: CGFloat = 14) -> Font
    static func caption(_ size: CGFloat = 12) -> Font
    static func code(_ size: CGFloat = 13) -> Font
}

enum CornerRadius {
    static let sm: CGFloat = 4;  static let md: CGFloat = 8
    static let lg: CGFloat = 12; static let xl: CGFloat = 16
    static let full: CGFloat = 9999
}

enum DesignAnimation {
    static func standard(_ reduceMotion: Bool) -> Animation?  // 0.2s
    static func fast(_ reduceMotion: Bool) -> Animation?      // 0.1s
    static func spring(_ reduceMotion: Bool) -> Animation?    // spring
}
```

Button styles: `DarkButtonStyle` (burgundy primary), `GhostButtonStyle` (transparent), `DestructiveButtonStyle` (red outline).

### State Layer

#### RepoState (`Concerto/State/RepoState.swift`)

Main application state, ~626 lines. Owns `WaveStore` and `RunStore`.

```swift
@MainActor @Observable
final class RepoState {
    var currentRepo: URL?
    var flows: [Flow]
    var availableDirections: [String]
    let waveStore: WaveStore
    let runStore: RunStore
    var waves: [WaveViewModel]         // derived from waveStore.ordered
    var waveGroups: WaveGroups         // derived from waveStore.groups
    var selectedWaveId: String?
    var selectedWave: WaveViewModel?
    var isLoading: Bool
    var errorMessage: String?
    var lfdConnected: Bool

    // Lifecycle
    func openRepo(_ url: URL, outputBuffer: OutputBuffer, skipBackgroundRefresh: Bool) async
    func refreshWaves() async
    func refreshFlowsAsync() async

    // Wave CRUD
    func createWave(name: String) async throws
    func deleteWave(_ wave: WaveViewModel) async throws
    func renameWave(_ wave: WaveViewModel, to: String) async throws
    func cloneWave(_ wave: WaveViewModel) async throws -> WaveViewModel
    func updateWave(_ wave: WaveViewModel, flow: String?, direction: [String]?, area: [String]?, prLimit: Int?, mergeMode: MergeMode?) async throws

    // Wave actions
    func runWave(wave: WaveViewModel, flow: String?, direction: [String]?, area: [String]?) async throws
    func stopWave(_ wave: WaveViewModel) async throws
    func landWave(_ wave: WaveViewModel) async throws
    func nextWave(_ wave: WaveViewModel) async throws
    func combinePRs(_ waveId: String) async throws -> CombinePRsResult

    // Stimuli
    func addStimulus(wave: WaveViewModel, kind: Stimulus.Kind, cron: String?) async throws
    func removeStimulus(wave: WaveViewModel, stimulusId: String) async throws

    // Runs
    func loadRuns(for waveId: String)
}
```

#### WaveStore (`Concerto/State/WaveStore.swift`)

Dictionary-backed store with automatic group computation (~186 lines).

```swift
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

    // Optimistic mutations with rollback
    func applyOptimistic(_ id: String, _ mutation: (inout WaveViewModel) -> Void) -> WaveViewModel?
    func commitMutation(_ id: String)
    func rollback(_ snapshot: WaveViewModel)

    // Pending create/delete
    func insertPending(_ wave: WaveViewModel)
    func replacePending(_ pendingId: String, with: WaveViewModel)
    func removePending(_ id: String)
    func applyDelete(_ id: String)

    func wave(for id: String) -> WaveViewModel?
}

struct WaveGroups {
    let blocked: [WaveViewModel]        // failed waves
    let pr: [WaveViewModel]             // waves with open PRs
    let recentActivity: [WaveViewModel] // active in last hour
    let active: [WaveViewModel]         // running/waiting
    let idle: [WaveViewModel]           // idle waves
    var attentionCount: Int
    var openPRCount: Int
    var allInOrder: [WaveViewModel]
}
```

#### RunStore (`Concerto/State/RunStore.swift`)

```swift
@MainActor @Observable
final class RunStore {
    private(set) var runs: [String: [WaveRun]]
    func setRuns(for waveId: String, _ newRuns: [WaveRun])
    func runs(for waveId: String) -> [WaveRun]
    func clear(for waveId: String)
}
```

#### OutputBuffer (`Concerto/State/OutputBuffer.swift`)

```swift
@MainActor @Observable
final class OutputBuffer {
    var interactiveSession: InteractiveSession?
    func appendOutput(waveId: String, text: String, timestamp: Date)
    func output(for waveId: String) -> [OutputLine]
    func clearOutput(for waveId: String)
    func startStreaming(waveId: String)
    func stopStreaming(waveId: String)
    func recentOutput(for waveId: String, maxLength: Int) -> String?
    func launchInteractiveSession(waveId: String, step: String, worktreePath: String, prompt: String?)
    func endInteractiveSession()
    func hasActiveSession(for waveId: String) -> Bool
}
```

### Services

#### SetupService (`Concerto/Services/SetupService.swift`)

```swift
struct SetupService {
    struct DependencyStatus { var lfInstalled: Bool; var lfPath: String? }
    func checkDependencies() -> DependencyStatus
    func install() async throws     // installs uv, loopflow, Node.js, Claude Code
    func ensureDaemonRunning() async throws
}
```

#### TerminalLauncher (`Concerto/Services/TerminalLauncher.swift`)

```swift
struct TerminalLauncher {
    func launchTerminal(_ terminal: TerminalApp, at path: URL, command: String?) throws
    func openInIDE(_ ide: IDEApp, at path: URL, workspace: String?) throws
    func openInFinder(at path: URL)
    func openURL(_ url: URL)
    func launchStep(_ step: String, terminal: TerminalApp, at repo: URL) throws
}
```

#### NameGenerator (`Concerto/Services/NameGenerator.swift`)

```swift
enum NameGenerator {
    static let magical: [String]    // 34 words: aurora, cascade, crystal...
    static let musical: [String]    // 26 words: allegro, aria, cadence...
    static func generate() -> String // "magical-musical"
}
```

#### RecentsService (`Concerto/Services/RecentsService.swift`)

```swift
@Observable final class RecentsService {
    private(set) var recentRepos: [RecentRepo]
    func addRecent(_ url: URL)
    func removeRecent(_ url: URL)
    func clearAll()
}
```

#### SnapshotService (`Concerto/Services/SnapshotService.swift`)

```swift
@MainActor struct SnapshotService {
    func snapshotKeyWindow() throws -> URL
    func snapshotKeyWindow(to outputPath: String) throws -> URL
    func snapshotWindow(_ window: NSWindow, to outputURL: URL) throws -> URL
}
```

#### RecentAreasService (`Concerto/Services/RecentAreasService.swift`)

```swift
struct RecentAreasService {
    func recentAreas(for repoURL: URL) -> [String]
    func addRecentArea(_ area: String, for repoURL: URL)
    func clearRecentAreas(for repoURL: URL)
}
```

### Ghostty Integration (`Concerto/Services/Ghostty/`)

Embedded terminal via GhosttyKit binary framework.

```swift
enum TerminalStatus: Equatable {
    case initializing
    case running
    case completed(exitCode: Int32)
    case failed(error: String)
}

struct GhosttySession: Identifiable {
    let id: String
    let worktree: String
    let command: [String]
    var status: TerminalStatus
    var surface: ghostty_surface_t?
}
```

### View Layer — Key Views

| View | File | Purpose |
|------|------|---------|
| `ConcertoApp` | `ConcertoApp.swift` | App entry, window groups, menu commands (⌘K, ⌘4) |
| `WelcomeWindow` | `Views/WelcomeWindow.swift` | First launch, recent repos, "Open Folder" |
| `RepoWindow` | `Views/RepoWindow.swift` | Per-repo window, setup check → ContentView |
| `SetupView` | `Views/SetupView.swift` | Dependency installation wizard |
| `ContentView` | `Views/ContentView.swift` | NavigationSplitView: sidebar + detail |
| `WaveSidebar` | `Views/WaveSidebar.swift` (~380 lines) | Grouped wave list, keyboard nav, create button |
| `WaveRow` | `Views/WaveRow.swift` | Sidebar row: status color, name, area, output preview |
| `WaveDetailPanel` | `Views/WaveDetailPanel.swift` (~690 lines) | Detail: Current tab (state-adaptive) + Runs tab |
| `StepRunner` | `Views/StepRunner.swift` | Flow/area/direction config + run button + stimuli |
| `CommandPalette` | `Views/CommandPalette.swift` | ⌘K fuzzy search: New Wave, Refresh, Open Terminal/IDE |
| `LiveOutput` | `Views/LiveOutput.swift` | Streaming agent output, auto-scroll |
| `InteractiveSessionView` | `Views/InteractiveSessionView.swift` | Embedded Ghostty terminal for interactive steps |
| `WaveRunsTab` | `Views/WaveRunsTab.swift` | Run history, PR combination |
| `QuickExperimentView` | `Views/QuickExperimentView.swift` | One-off step launch without wave |
| `DiagnosticsView` | `Views/DiagnosticsView.swift` | Log viewer |

**Typeaheads:** `AreaTypeahead`, `DirectionTypeahead`, `FlowTypeahead`, `TypeaheadComponents` — shared fuzzy-search picker pattern.

**WaveSidebar sections:** Needs Attention (failed), Open PRs, Recent Activity, Active (running/waiting), Idle.

**WaveDetailPanel adapts by state:**
- Idle → StepRunner + commit log + Land/Next actions
- Running → progress pills + live output
- Failed → error display + retry

### App Entry (`Concerto/ConcertoApp.swift`)

```swift
@main struct ConcertoApp: App {
    @State private var recentsService = RecentsService()
    var body: some Scene {
        WindowGroup { /* Welcome or RepoWindow */ }
        WindowGroup(id: "repo", for: URL.self) { $repoURL in
            RepoWindow(repoURL: repoURL, recentsService: recentsService)
        }
        Window("Terminal Test", id: "terminal-test") { TerminalTestWindow() }
        .commands { /* Beta features, Appearance, ⌘K, ⌘4 snapshot */ }
    }
}
```

---

## Key Patterns

### Optimistic UI
`WaveStore` supports `applyOptimistic()` → server call → `commitMutation()` or `rollback()`. Pending mutations block incoming WebSocket events from overwriting optimistic state.

### Actor Isolation
- `@MainActor` on all UI state: `RepoState`, `WaveStore`, `RunStore`, `OutputBuffer`
- `actor LocalEventService` for WebSocket thread safety
- Services are `Sendable`

### Communication
1. **HTTP** (LocalWaveService) — CRUD, actions, discovery
2. **WebSocket** (LocalEventService) — live wave/worktree/output events
3. **Notifications** (NotificationService) — interactive needed, errors, PR ready

### Design System Enforcement
All spacing via `Spacing` enum, colors via `LoopflowPalette` / semantic `statusX` colors, typography via `Typography` helpers. Reduce-motion respected via `DesignAnimation`. Hit targets enforced via `.minHitTarget()`.

---

## File Map

```
swift/
├── Package.swift
├── README.md
├── DESIGN.md                           # design principles (~25KB)
├── LoopflowCore/
│   ├── LoopflowCore.swift              # lfd port/URL constants
│   ├── Models/
│   │   ├── Wave.swift                  # Wave, WaveStatus, Stimulus, CommitEntry
│   │   ├── WaveRun.swift               # WaveRun, WaveRunStatus
│   │   ├── WaveViewModel.swift         # Enriched client model
│   │   ├── Flow.swift                  # Flow, FlowType
│   │   ├── Step.swift                  # Step, StepConfig, StepRun
│   │   ├── PullRequest.swift           # PullRequest, PRState
│   │   ├── AppPreferences.swift        # TerminalApp, IDEApp enums
│   │   ├── AppearanceMode.swift        # system/light/dark
│   │   └── StatusColors.swift          # Color extensions
│   └── Services/
│       ├── LocalWaveService.swift      # HTTP API client
│       ├── LocalEventService.swift     # WebSocket client
│       ├── WaveServiceProtocol.swift   # Service interface
│       ├── AuthService.swift           # OAuth + Keychain
│       ├── AuthState.swift             # Observable auth state
│       ├── AuthError.swift             # Auth error types
│       ├── TokenProvider.swift         # Token protocol
│       ├── LoggingService.swift        # File logging
│       └── NotificationService.swift   # macOS notifications
├── Concerto/
│   ├── ConcertoApp.swift              # @main, window groups
│   ├── BrandColors.swift              # Burgundy, cream palette
│   ├── DesignSystem.swift             # Spacing, Typography, buttons
│   ├── Flags.swift                    # Feature flags
│   ├── ScriptCommands.swift           # AppleScript handlers
│   ├── Models/
│   │   └── RecentRepo.swift
│   ├── Services/
│   │   ├── SetupService.swift         # Dependency installer
│   │   ├── TerminalLauncher.swift     # Terminal/IDE launcher
│   │   ├── NameGenerator.swift        # Random wave names
│   │   ├── RecentsService.swift       # Recent repos
│   │   ├── RecentAreasService.swift   # Per-repo area history
│   │   ├── SnapshotService.swift      # Window screenshots
│   │   ├── AppIconProvider.swift      # App icon lookup
│   │   └── Ghostty/
│   │       ├── GhosttyManager.swift
│   │       ├── GhosttyTerminalView.swift
│   │       └── GhosttyTypes.swift
│   ├── State/
│   │   ├── RepoState.swift            # Main app state (~626 lines)
│   │   ├── WaveStore.swift            # Wave storage + groups (~186 lines)
│   │   ├── RunStore.swift             # Run cache
│   │   └── OutputBuffer.swift         # Output streaming + interactive sessions
│   └── Views/
│       ├── ContentView.swift          # Main split view
│       ├── WaveSidebar.swift          # Grouped wave list (~380 lines)
│       ├── WaveRow.swift              # Sidebar row
│       ├── WaveDetailPanel.swift      # Detail view (~690 lines)
│       ├── StepRunner.swift           # Run configuration
│       ├── CommandPalette.swift       # ⌘K search
│       ├── LiveOutput.swift           # Streaming output
│       ├── InteractiveSessionView.swift # Embedded terminal
│       ├── WaveRunsTab.swift          # Run history
│       ├── WelcomeWindow.swift        # Launch screen
│       ├── RepoWindow.swift           # Repo window wrapper
│       ├── SetupView.swift            # Install wizard
│       ├── QuickExperimentView.swift  # One-off step runner
│       ├── DiagnosticsView.swift      # Log viewer
│       ├── AreaTypeahead.swift
│       ├── DirectionTypeahead.swift
│       ├── FlowTypeahead.swift
│       ├── TypeaheadComponents.swift
│       ├── FlowProgressPills.swift
│       ├── IterationTimeline.swift
│       ├── NextActionsBar.swift
│       ├── WaitingStateCard.swift
│       ├── EmbeddedTerminalPanel.swift
│       ├── ScreenshotWindow.swift
│       ├── TerminalTestWindow.swift
│       └── ThemePreview.swift
├── Symphonia/                          # placeholder
├── ConcertoTests/
│   ├── WaveStoreTests.swift           # ~425 lines, optimistic mutations
│   ├── RunStoreTests.swift
│   ├── AuthServiceTests.swift
│   ├── GhosttyTests.swift
│   ├── WaveRowTests.swift
│   └── WaveTests.swift
├── ConcertoUITests/
│   └── ScreenshotPipelineTests.swift
└── SymphoniaTests/
    └── SymphoniaTests.swift
```

---

## Testing

```bash
swift test --package-path swift        # all tests
swift test --package-path swift --filter ConcertoTests  # unit tests only
```

- **WaveStoreTests** (~425 lines): optimistic mutations, group computation, pending state guards, rollback
- **RunStoreTests**: caching, capacity limits
- **AuthServiceTests**: sign-in/out flows
- **WaveRowTests**: ViewInspector UI component tests
- **WaveTests**: model encoding/decoding
- **GhosttyTests**: terminal integration
- **ScreenshotPipelineTests**: visual regression via snapshots
