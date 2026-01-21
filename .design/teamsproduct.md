# Rebrand: Concerto + Symphonia + Web

**What to build:** Rename Maestro → Concerto, restructure into `LoopflowSwift/` monorepo with shared core, stub Symphonia (teams product), add `loopflow-web/` as web client.

## Background

Maestro conflicts with Conductor (competitor). New naming follows the musical metaphor:

- **Concerto** — Solo performer featured with orchestra. Individual dev + agents. Free, open source forever.
- **Symphonia** — Full orchestral work. Teams of engineers playing together. Client open source, server private.

From teams-vision.md: "Each musician brings their own instrument (their Maestro + agents)." Concerto *is* that instrument. Symphonia coordinates the ensemble.

## Data Structures

No new models. Existing models move to shared core:

```
LoopflowSwift/
├── Package.swift              # workspace manifest
├── LoopflowCore/              # shared library
│   ├── Models/
│   │   ├── Worktree.swift
│   │   ├── Session.swift
│   │   ├── Loop.swift
│   │   ├── Pipeline.swift
│   │   └── ...                # all current models
│   └── Services/
│       ├── WorktreeService.swift
│       ├── SessionService.swift
│       ├── LoopService.swift
│       ├── LFDEventService.swift
│       └── ...                # most current services
├── Concerto/                  # individual dev app (was Maestro)
│   ├── ConcertoApp.swift
│   ├── Views/
│   │   └── ...                # all current views
│   └── Services/
│       └── ...                # UI-specific services only
├── Symphonia/                 # teams app (stub)
│   ├── SymphoniaApp.swift
│   └── Views/
│       └── PlaceholderView.swift
├── ConcertoTests/
└── SymphoniaTests/
```

## loopflow-web

Web client port of LoopflowSwift. Next.js + TypeScript on Vercel.

```
loopflow-web/
├── package.json
├── next.config.js
├── tsconfig.json
├── src/
│   ├── app/
│   │   ├── layout.tsx
│   │   ├── page.tsx              # welcome/repo picker
│   │   └── repo/
│   │       └── [path]/
│   │           └── page.tsx      # main workspace view
│   ├── components/
│   │   ├── WorktreeSidebar.tsx
│   │   ├── PromptLauncher.tsx
│   │   ├── OutputPanel.tsx
│   │   └── LoopStatus.tsx
│   ├── models/
│   │   ├── worktree.ts
│   │   ├── session.ts
│   │   ├── loop.ts
│   │   └── pipeline.ts
│   ├── services/
│   │   ├── lfd-client.ts         # WebSocket to lfd proxy
│   │   ├── symphonia-client.ts   # REST/WebSocket to Symphonia API
│   │   ├── worktree-service.ts
│   │   └── session-service.ts
│   └── lib/
│       └── api.ts
└── public/
```

### Two modes

**Local mode (like Concerto):**
- Talks to `lfd` via WebSocket proxy
- `lfd` needs HTTP/WebSocket endpoint (new: `lfd serve --http`)
- Full functionality: launch tasks, view output, manage worktrees

**Teams mode (like Symphonia):**
- Talks to Symphonia server API
- Team visibility, shared loops, cross-engineer coordination
- Server handles lfd communication

### lfd HTTP bridge

Add HTTP/WebSocket transport to lfd:

```python
# loopflow/lfd/http.py
@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    """Bridge WebSocket to Unix socket protocol"""
    ...

@app.get("/api/status")
async def status():
    """REST endpoint for status"""
    ...
```

```bash
lfd serve --http --port 8765    # enables HTTP + WebSocket
```

Web client connects to `ws://localhost:8765/ws`.

### README.md

```markdown
# loopflow-web

Web client for Loopflow. Port of LoopflowSwift.

## Philosophy

This is a **follower, not a leader**. Design decisions happen in the Swift app first; this port translates them to web.

Goals:
- **Portable patterns** — Standard Next.js/React/TypeScript. No exotic dependencies. Easy for any web dev to pick up.
- **Mirror Swift** — Components, models, and services map 1:1 to LoopflowSwift where possible.
- **Don't innovate here** — If something needs design work, do it in Swift first, then port.

This may take on its own life at later maturity. For now, it follows.

## Development

\`\`\`bash
npm install
npm run dev     # http://localhost:3000
\`\`\`

## Modes

- **Local mode** — Connects to `lfd serve --http` on your machine
- **Teams mode** — Connects to Symphonia server API
```

### Shared types

TypeScript models mirror Swift/Python:

```typescript
// src/models/worktree.ts
interface Worktree {
  path: string;
  branch: string;
  isMain: boolean;
  status: WorktreeStatus;
}

// src/models/session.ts
interface Session {
  id: string;
  task: string;
  status: 'running' | 'completed' | 'error';
  startedAt: string;
  endedAt?: string;
}
```

## Key Decisions

### What's shared (LoopflowCore)

Everything that both apps need:

```swift
// Models - all of them
Worktree, Session, Loop, Pipeline, PromptCard, Voice,
LoopflowConfig, ContextPreview, FileDiffStat, CommitInfo,
SessionResult, WorkItem

// Services - business logic
WorktreeService     // git worktree operations
SessionService      // session management
LoopService         // lfd loop operations
LFDEventService     // socket client for lfd
ConfigLoader        // .lf/config.yaml parsing
PromptService       // prompt discovery
PipelineService     // pipeline definitions
ContextPreviewService
TokenEstimator
VoiceService
WorkService
ResultsService
```

### What stays app-specific

UI-bound services that differ per product:

```swift
// Concerto only (for now)
RecentsService      // recent repos (individual)
SetupService        // onboarding flow
TerminalLauncher    // local terminal integration
CaptureService      // screenshot capture
AppIconProvider     // dock icon
LoggingService      // local logging
NameGenerator       // worktree name generation
```

Symphonia will eventually have its own versions (team recents, team setup, etc.).

### What Symphonia will add (future, not this PR)

```swift
// Future Symphonia-specific
TeamService         // team membership, permissions
TeamWorktreeService // cross-engineer visibility
SharedLoopService   // team-wide loop coordination
```

## File Operations

### Rename/move

```
Maestro/                    → LoopflowSwift/
Maestro/Maestro/            → LoopflowSwift/Concerto/
Maestro/MaestroTests/       → LoopflowSwift/ConcertoTests/
Maestro/MaestroUITests/     → (delete or move to ConcertoUITests/)
```

### Extract to LoopflowCore

```
Concerto/Models/*           → LoopflowCore/Models/
Concerto/Services/*         → LoopflowCore/Services/ (most)
```

Keep in Concerto:
- `RecentsService`, `SetupService`, `TerminalLauncher`
- `CaptureService`, `AppIconProvider`, `LoggingService`, `NameGenerator`

### String replacements

```
"Maestro"  → "Concerto"     (product name)
"maestro"  → "concerto"     (bundle id, paths)
MaestroApp → ConcertoApp    (type name)
```

## Package.swift

```swift
// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "LoopflowSwift",
    platforms: [.macOS(.v15)],
    products: [
        .library(name: "LoopflowCore", targets: ["LoopflowCore"]),
        .executable(name: "Concerto", targets: ["Concerto"]),
        .executable(name: "Symphonia", targets: ["Symphonia"]),
    ],
    dependencies: [
        .package(url: "https://github.com/nalexn/ViewInspector.git", from: "0.10.0")
    ],
    targets: [
        .target(
            name: "LoopflowCore",
            path: "LoopflowCore"
        ),
        .executableTarget(
            name: "Concerto",
            dependencies: ["LoopflowCore"],
            path: "Concerto"
        ),
        .executableTarget(
            name: "Symphonia",
            dependencies: ["LoopflowCore"],
            path: "Symphonia"
        ),
        .testTarget(
            name: "ConcertoTests",
            dependencies: ["Concerto", "LoopflowCore", "ViewInspector"],
            path: "ConcertoTests"
        ),
        .testTarget(
            name: "SymphoniaTests",
            dependencies: ["Symphonia", "LoopflowCore"],
            path: "SymphoniaTests"
        ),
    ]
)
```

## Constraints

- **Git history:** Use `git mv` for renames to preserve history
- **Bundle identifier:** `com.loopflow.concerto` (was `com.loopflow.maestro`)
- **Python CLI unchanged:** `lf`, `lfops`, `lfd` stay as "loopflow"
- **Docs:** Update `docs/next/maestro.md` → reference Concerto

## UI Changes

This is infrastructure. No Maestro UI changes needed.

Update documentation references:
- `docs/next/maestro.md` → mentions Concerto
- `.docs/maestro-vision.md` → rename to `concerto-vision.md`
- `.docs/teams-vision.md` → update metaphor table

## Done When

```bash
# Swift builds succeed
cd LoopflowSwift && swift build

# Both Swift apps launch
swift run Concerto    # opens individual app
swift run Symphonia   # opens stub/placeholder

# Swift tests pass
swift test

# No Maestro references remain (except git history)
grep -r "Maestro" LoopflowSwift/ --include="*.swift" | grep -v "// was Maestro"
# (should return empty)

# Web client builds
cd loopflow-web && npm install && npm run build

# Web client dev server runs
npm run dev           # opens at localhost:3000
```

## Build System

Keep XcodeGen. Structure `project.yml` for monorepo:

```yaml
name: LoopflowSwift
targets:
  LoopflowCore:
    type: framework
    platform: macOS
    sources: LoopflowCore
  Concerto:
    type: application
    platform: macOS
    sources: Concerto
    dependencies:
      - target: LoopflowCore
    info:
      path: Concerto/Info.plist
  Symphonia:
    type: application
    platform: macOS
    sources: Symphonia
    dependencies:
      - target: LoopflowCore
    info:
      path: Symphonia/Info.plist
  ConcertoTests:
    type: bundle.unit-test
    platform: macOS
    sources: ConcertoTests
    dependencies:
      - target: Concerto
      - target: LoopflowCore
  SymphoniaTests:
    type: bundle.unit-test
    platform: macOS
    sources: SymphoniaTests
    dependencies:
      - target: Symphonia
      - target: LoopflowCore
```

## Decided

### Hierarchical AppState

Shared base in LoopflowCore, app-specific extensions:

```swift
// LoopflowCore/AppState.swift
@Observable
class AppState {
    // Shared state
    var currentRepo: URL?
    var worktrees: [Worktree] = []
    var sessions: [Session] = []
    var loops: [Loop] = []

    // Shared services
    let worktreeService: WorktreeService
    let sessionService: SessionService
    let loopService: LoopService
    let lfdClient: LFDClient

    init(lfdClient: LFDClient) {
        self.lfdClient = lfdClient
        self.worktreeService = WorktreeService()
        self.sessionService = SessionService(lfdClient: lfdClient)
        self.loopService = LoopService(lfdClient: lfdClient)
    }
}

// Concerto/ConcertoAppState.swift
@Observable
class ConcertoAppState: AppState {
    // Individual-specific
    var recentRepos: [RecentRepo] = []
    let recentsService: RecentsService

    override init(lfdClient: LFDClient) {
        self.recentsService = RecentsService()
        super.init(lfdClient: lfdClient)
    }
}

// Symphonia/SymphoniaAppState.swift
@Observable
class SymphoniaAppState: AppState {
    // Team-specific
    var teamMembers: [TeamMember] = []
    var teamWorktrees: [TeamWorktree] = []  // cross-engineer visibility
    let teamService: TeamService

    override init(lfdClient: LFDClient) {
        self.teamService = TeamService()
        super.init(lfdClient: lfdClient)
    }
}
```

### Platform strategy

**Swift = optimized.** Unix sockets, streaming, native performance. The flagship experience.

**Web = simple.** HTTP API, standard patterns, portable. Good enough, easy to maintain.

```swift
// LoopflowCore/Services/LFDClient.swift
protocol LFDClient {
    func status() async throws -> DaemonStatus
    func listWorktrees() async throws -> [Worktree]
    func subscribe(events: [String]) -> AsyncStream<LFDEvent>
}

// LoopflowCore/Services/LFDSocketClient.swift
class LFDSocketClient: LFDClient {
    /// Unix socket at ~/.lf/lfd.sock — used by Swift apps
    private let socketPath: String
    ...
}
```

```typescript
// loopflow-web/src/services/lfd-client.ts
// Simple HTTP + WebSocket, no fancy optimizations
const status = await fetch('/api/status').then(r => r.json())
const ws = new WebSocket('ws://localhost:8765/ws')
```

Both speak the same protocol. Swift gets the fast path; web gets the simple path.

## Open Questions

1. **Web client naming?** — `loopflow-web` is descriptive but not musical. Alternatives: `Aria` (solo melody), `Prelude`, or just keep it technical?
