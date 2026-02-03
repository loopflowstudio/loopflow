# WaveService Protocol Abstraction

Abstract the transport layer so Concerto works with Python lfd (HTTP+socket), Rust lfd (gRPC), and remote lfd (HTTPS+auth).

## Problem

Concerto currently hardcodes transport details throughout the codebase:

- `WaveService.swift` makes HTTP calls to `http://127.0.0.1:8765`
- `LFDEventService.swift` connects to Unix socket at `~/.lf/lfd.sock`
- `LFDClient.swift` duplicates some HTTP calls
- Direct SQLite reads for FlowRuns bypass the daemon entirely

This coupling prevents:
1. Switching to Rust lfd's gRPC endpoints (already implemented, 40+ RPCs)
2. Remote access from mobile (Phase 3 goal)
3. Testing with mock backends

The user benefits from protocol abstraction because it unlocks mobile access—managing waves from iPhone/iPad while lfd runs on their Mac.

## Approach

Introduce a `WaveServiceProtocol` that captures all wave operations. Current code becomes `LocalWaveService`. Future implementations slot in without UI changes.

```swift
// Protocol defines what the app needs from lfd
protocol WaveServiceProtocol: Sendable {
    // Waves
    func listWaves(repo: URL) async throws -> [Wave]
    func createWave(name: String, repo: URL) async throws -> Wave
    func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave
    func deleteWave(_ id: String) async throws
    func cloneWave(_ id: String, name: String?) async throws -> Wave

    // Wave control
    func run(_ id: String, overrides: RunOverrides?) async throws
    func stop(_ id: String) async throws
    func connect(_ id: String) async throws -> ConnectionInfo

    // FlowRuns
    func listFlowRuns(waveId: String?, repo: URL?, limit: Int) async throws -> [FlowRun]

    // Collapse
    func collapsePRs(_ id: String) async throws -> CollapsePRsResult
}

// Event streaming as a separate protocol (different lifecycle)
protocol EventServiceProtocol: Sendable {
    func subscribe(
        patterns: [String],
        onEvent: @escaping @Sendable (LFDEvent) -> Void,
        onConnectionChange: @escaping @Sendable (Bool) -> Void
    ) async
    func disconnect()
    var isConnected: Bool { get async }
}
```

### Supporting types

```swift
struct WaveConfigUpdate {
    var name: String?
    var area: [String]?
    var direction: [String]?
    var flow: String?
    var stimulus: Stimulus?
    var paused: Bool?
}

struct RunOverrides {
    var area: [String]?
    var direction: [String]?
    var flow: String?
    var stimulus: Stimulus?
}

struct ConnectionInfo {
    let worktree: String
    let step: String
    let agentId: String
    let promptFile: String
    let waveRunId: String?
    let stepIndex: Int
}
```

### Implementations

| Implementation | Transport | Use case |
|----------------|-----------|----------|
| `LocalWaveService` | HTTP + Unix socket | Current Python lfd |
| `GRPCWaveService` | gRPC | Future Rust lfd |
| `RemoteWaveService` | HTTPS + auth tokens | Mobile to hosted lfd |

### Migration path

1. Extract protocol from current `WaveService` API surface
2. Rename `WaveService` to `LocalWaveService`, conform to protocol
3. Create `WaveServiceFactory` that returns appropriate implementation
4. Update `RepoState` to use factory
5. Delete `LFDClient.swift` (redundant with protocol)

Factory logic:
```swift
struct WaveServiceFactory {
    static func create(for context: Context) -> any WaveServiceProtocol {
        switch context {
        case .local:
            return LocalWaveService()
        case .grpc(let address):
            return GRPCWaveService(address: address)
        case .remote(let endpoint, let token):
            return RemoteWaveService(endpoint: endpoint, token: token)
        }
    }
}
```

### Event streaming

Events need separate handling because:
- Different connection lifecycle (persistent vs request/response)
- Different reconnection semantics per transport
- gRPC uses `Subscribe()` returning `stream Event`, not socket

`LocalEventService` wraps current Unix socket approach. `GRPCEventService` uses the streaming RPC. Both conform to `EventServiceProtocol`.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep HTTP for Rust lfd | Simpler, no new dependencies | Rust lfd already has gRPC, HTTP would be duplicate effort |
| Protocol-only, no factory | Less code | Factory needed for runtime switching (local vs remote) |
| Merge events into main protocol | Simpler API surface | Event streaming has different lifecycle, would complicate protocol |
| Abstract at view model level | Higher-level abstraction | Transport details leak into RepoState; better to abstract at service layer |

## Key decisions

1. **Separate protocol for events.** Event streaming has persistent connection semantics that don't fit request/response patterns. A separate protocol makes lifecycle explicit.

2. **Factory pattern for implementation selection.** Mobile will need to switch between local and remote dynamically. Factory centralizes this logic.

3. **Delete LFDClient.** It duplicates `WaveService` functionality. The protocol makes it redundant—`checkAvailability()` becomes a method on `WaveServiceProtocol`.

4. **Move SQLite reads behind the protocol.** Current direct SQLite access in `listFlowRuns` bypasses the daemon. The protocol forces all data through the daemon, which matters for remote access.

5. **Use `any WaveServiceProtocol` for existential.** Swift 6 requirement. The factory returns an existential; callers don't care about concrete type.

## Scope

**In scope:**
- Protocol definition for wave operations
- Protocol definition for event streaming
- `LocalWaveService` implementation (current HTTP behavior)
- `LocalEventService` implementation (current socket behavior)
- Factory for implementation selection
- Update `RepoState` to use protocol

**Out of scope:**
- `GRPCWaveService` implementation (Phase 2 follow-up)
- `RemoteWaveService` implementation (Phase 3)
- grpc-swift dependency (added when implementing GRPCWaveService)
- Authentication (Phase 2: loopflow-auth)
- Terminal streaming (Phase 2: grpc-terminal-streaming)

## Done when

1. `WaveServiceProtocol` and `EventServiceProtocol` defined in LoopflowCore
2. `LocalWaveService` conforms to `WaveServiceProtocol`, passes existing behavior
3. `LocalEventService` conforms to `EventServiceProtocol`
4. `RepoState` uses protocols via factory
5. `LFDClient.swift` deleted
6. All existing UI tests pass with no changes
7. No HTTP URLs or socket paths appear in `RepoState.swift`
