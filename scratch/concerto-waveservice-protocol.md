# WaveService Protocol Abstraction

Transport-agnostic service layer for Concerto. Phase 1 complete; gRPC and remote implementations are Phase 2/3.

## Current State

Phase 1 shipped:
- `WaveServiceProtocol` and `EventServiceProtocol` in LoopflowCore
- `LocalWaveService` (HTTP) and `LocalEventService` (socket) conform to protocols
- `WaveServiceFactory` for implementation selection
- `/wave-runs` HTTP endpoint replaces direct SQLite reads
- `LFDClient.swift` deleted (redundant)

## What's Next

| Implementation | Transport | Phase | Depends on |
|----------------|-----------|-------|------------|
| `GRPCWaveService` | gRPC | 2 | grpc-swift dependency |
| `GRPCEventService` | gRPC stream | 2 | grpc-swift dependency |
| `RemoteWaveService` | HTTPS + auth | 3 | loopflow-auth |

### GRPCWaveService (Phase 2)

Rust lfd already has 40+ gRPC RPCs. Swift implementation needs:
1. Add grpc-swift dependency
2. Generate Swift from `.proto` files
3. Implement `GRPCWaveService` conforming to `WaveServiceProtocol`
4. Implement `GRPCEventService` using `Subscribe()` streaming RPC

### RemoteWaveService (Phase 3)

Mobile access to lfd running on user's Mac:
1. Depends on loopflow-auth (GitHub OAuth)
2. Depends on lfd-registration (daemon registers with Loopflow service)
3. HTTPS transport with auth tokens

## Protocol Reference

```swift
protocol WaveServiceProtocol: Sendable {
    func listWaves(repo: URL) async throws -> [Wave]
    func createWave(name: String, repo: URL) async throws -> Wave
    func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave
    func deleteWave(_ id: String) async throws
    func cloneWave(_ id: String, name: String?) async throws -> Wave
    func run(_ id: String, overrides: RunOverrides?) async throws
    func stop(_ id: String) async throws
    func connect(_ id: String) async throws -> ConnectionInfo
    func listWaveRuns(waveId: String?, repo: URL?, limit: Int) async throws -> [WaveRun]
    func collapsePRs(_ id: String) async throws -> CollapsePRsResult
}

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

## Key Decisions

1. **Separate protocol for events.** Persistent connection lifecycle doesn't fit request/response.
2. **Factory pattern.** Mobile needs runtime switching between local and remote.
3. **All data through daemon.** No direct SQLite—required for remote access.
