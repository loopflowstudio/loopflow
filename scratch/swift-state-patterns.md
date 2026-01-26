# Swift State Patterns: Tradeoffs

Mini design doc comparing approaches for splitting AppState.

## Context

Concerto uses a single `@Observable` AppState class (1,100 lines). Splitting it into focused objects requires choosing an injection pattern.

## Options

### Option 1: @Environment (Recommended)

```swift
@main
struct ConcertoApp: App {
    @State private var repoState = RepoState()
    @State private var sessionState = SessionState()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(repoState)
                .environment(sessionState)
        }
    }
}

struct AgentSidebar: View {
    @Environment(RepoState.self) private var repoState
    // Only observes RepoState changes
}
```

**Pros:**
- SwiftUI-native, standard pattern
- Views only observe what they declare
- Easy to test views with mock state
- No coupling between state objects

**Cons:**
- Must thread environment through view hierarchy
- Can't access outside views (need to pass explicitly to services)
- Slightly more boilerplate at injection site

### Option 2: Explicit parameters

```swift
struct AgentSidebar: View {
    let repoState: RepoState  // passed from parent

    var body: some View { ... }
}
```

**Pros:**
- Completely explicit dependencies
- Easiest to test
- Works anywhere (views, services, etc.)

**Cons:**
- Verbose—every parent must pass down
- Refactoring requires updating call sites
- Prop drilling through deep hierarchies

### Option 3: Shared container (AppState stays, but delegates)

```swift
@Observable final class AppState {
    let repo = RepoState()
    let session = SessionState()
    let launcher = LauncherState()
}

struct AgentSidebar: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        List(appState.repo.agents) { ... }
    }
}
```

**Pros:**
- Single injection point
- State objects can reference each other via parent
- Gradual migration—add delegation without changing injection

**Cons:**
- Still one god object at root
- Views observe entire container (though @Observable is smart about nested observation)
- Harder to test individual state in isolation

### Option 4: Dependency injection container (Swinject-style)

```swift
let container = Container()
container.register(RepoState.self) { _ in RepoState() }
container.register(SessionState.self) { _ in SessionState() }

// In views or anywhere
@Injected var repoState: RepoState
```

**Pros:**
- Decoupled from SwiftUI
- Works in services, models, anywhere
- Mature pattern from other platforms

**Cons:**
- External dependency or custom implementation
- Runtime resolution (not compile-time safe)
- Less SwiftUI-idiomatic
- Adds abstraction layer

## Recommendation

**Use @Environment (Option 1)** for views, with explicit parameters for services.

```swift
// Views use environment
struct AgentSidebar: View {
    @Environment(RepoState.self) private var repoState
}

// Services take state as parameters
class AgentService {
    func refresh(into state: RepoState) async throws {
        let agents = try await fetchAgents()
        await MainActor.run { state.agents = agents }
    }
}

// Coordination happens in views
struct ContentView: View {
    @Environment(RepoState.self) private var repoState
    let agentService = AgentService()

    func refresh() async {
        try? await agentService.refresh(into: repoState)
    }
}
```

This is:
- Standard SwiftUI pattern
- Testable (inject mock state)
- No external dependencies
- Clear data flow (state flows down, actions flow up)

## Cross-state communication

When one state object needs to affect another (e.g., session ends → update agent status):

```swift
// Option A: Coordinator view handles it
struct ContentView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(SessionState.self) private var sessionState

    func onSessionEnd(id: String) {
        // Update session
        sessionState.activeSessions.removeValue(forKey: id)
        // Update agent
        if let agent = repoState.agents.first(where: { ... }) {
            // trigger refresh or update
        }
    }
}

// Option B: Callback/closure passed to session state
sessionState.onSessionEnd = { [weak repoState] sessionId in
    repoState?.refreshAgent(...)
}
```

Option A is cleaner—keep coordination in views, state objects stay dumb.

## Migration strategy

1. Create new state classes
2. Add them to environment alongside existing AppState
3. Gradually move properties from AppState to new classes
4. Update views one at a time
5. Delete AppState when empty

This allows incremental migration without big-bang refactor.
