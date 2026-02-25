# Portfolio View for Concerto

Replace the Welcome window with a Portfolio — a single pane showing all your repos and their waves, so you can see where attention is needed most.

## What to build

The Welcome window becomes a portfolio dashboard. Each repo is a card in a grid. Each card shows the repo's waves with sidebar-level density (status, diff stats, activity). A blank `+` card opens a typeahead to add repos from `~/src/`. Clicking a wave opens (or focuses) the repo window with that wave selected.

## Data structures

```swift
// Portfolio is just the list of repos the user has opened.
// RecentsService already persists this — rename/extend to PortfolioService.
@Observable
final class PortfolioService {
    private(set) var repos: [PortfolioRepo]

    func addRepo(_ url: URL)
    func removeRepo(_ url: URL)
    func clearAll()
}

struct PortfolioRepo: Identifiable, Codable {
    let path: String
    var lastOpened: Date

    var id: String { path }
    var displayName: String  // last path component
    var url: URL
    var exists: Bool
}
```

The portfolio needs wave data for each repo. Each card fetches its own waves from lfd:

```swift
// Per-repo state, lighter than full RepoState.
// Just enough to show the card — waves + connection status.
@Observable
final class PortfolioRepoState {
    let repo: PortfolioRepo
    let connection: ServerConnection

    private(set) var waves: [WaveViewModel] = []
    private(set) var isConnected: Bool = false
    private(set) var isLoading: Bool = true

    func connect() async
    func refresh() async
}
```

Summary stats derived from waves:

```swift
extension PortfolioRepoState {
    var activeCount: Int      // running + waiting
    var blockedCount: Int     // waiting waves
    var totalDiffLines: Int   // sum of diff stats across waves
    var needsAttention: Bool  // any failed or waiting waves
}
```

## Key views

```
PortfolioWindow (replaces WelcomeWindow)
├── LazyVGrid of PortfolioRepoCard
│   ├── Repo display name (header)
│   ├── Summary: "3 waves · 1 blocked · +284 -91"
│   ├── Mini wave list (compact WaveRow variant)
│   │   ├── status dot + name + diff stat
│   │   └── ... (max ~5 visible, scroll for more)
│   └── Connection indicator
├── AddRepoCard (+)
│   └── on click → typeahead overlay
└── Typeahead overlay
    ├── TextField with live filtering
    ├── Scans ~/src/ for git repos (main worktrees only)
    └── Select → addRepo → card appears
```

### PortfolioRepoCard

```swift
struct PortfolioRepoCard: View {
    let repoState: PortfolioRepoState
    let onSelectWave: (String, URL) -> Void  // waveId, repoURL
    let onOpenRepo: (URL) -> Void
}
```

Each card is a square-ish box on the burgundy background. Contains:
- **Header**: repo name (bold), connection dot
- **Summary line**: wave count, blocked count, total diff
- **Wave list**: compact rows — status dot, name, `+N -M` diff, PR badge if open
- **Click wave** → opens repo window focused on that wave
- **Click header** → opens repo window (no wave selected)

### AddRepoCard

Visually a dashed-border card with `+`. On click, shows a typeahead:
- Scans `~/src/` (and maybe other common dirs) for directories containing `.git/`
- Filters to "main worktrees" (not `.git/worktrees/` children)
- Typeahead filters as you type
- Select → `portfolioService.addRepo(url)` → card appears immediately

### Typeahead implementation

```swift
struct RepoTypeahead: View {
    @State private var query = ""
    @State private var candidates: [URL] = []
    let onSelect: (URL) -> Void

    // On appear, scan ~/src/ for git repos
    // Filter candidates by query (fuzzy match on directory name)
}
```

Scanning strategy: `FileManager` enumeration of `~/src/` one level deep, filtering to directories that contain `.git/`. This is fast for a flat `~/src/` layout. If needed later, make the scan root configurable.

## Lifecycle

1. App launches → `PortfolioWindow` shown (instead of `WelcomeWindow`)
2. For each repo in portfolio, create a `PortfolioRepoState` and connect to lfd
3. Wave data streams in via lfd HTTP API (same `listWaves` endpoint)
4. WebSocket subscriptions for live updates per repo (wave status changes)
5. User clicks wave → `openWindow(id: "repo", value: repoURL)` + post notification to select wave
6. User clicks `+` → typeahead → select repo → added to portfolio, card appears

## Constraints

- **lfd connection**: all repos go through the same lfd instance (current `ConnectionStore` model). The portfolio doesn't need per-repo connections — lfd already serves multiple repos.
- **RecentsService → PortfolioService**: RecentsService is already close to what we need. Extend it rather than building from scratch. The 10-repo max should increase or be removed.
- **Wave selection across windows**: when clicking a wave in the portfolio to open a repo window, the repo window needs to know which wave to select. This may need a notification with waveId, or passing it through the window open mechanism.

## What not to build

- Cross-repo waves or operations
- Wave creation from the portfolio (do that in the repo window)
- Drag-and-drop reordering of repos
- Multiple lfd connections (one server for now)

## Done when

1. App launches to portfolio grid instead of welcome screen
2. Each repo card shows waves with status dots, names, and diff counts
3. Summary line shows blocked wave count per repo
4. Clicking a wave opens the repo window with that wave selected
5. `+` card opens typeahead over `~/src/`, selecting adds repo to portfolio
6. Repos persist across launches (like recents do now)
7. Removing a repo from portfolio works (context menu or similar)
8. Live wave status updates reflected in portfolio cards
