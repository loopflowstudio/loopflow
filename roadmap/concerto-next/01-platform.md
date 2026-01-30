# Platform & Server Strategy

## Platform Story

| Platform | Conduct | Improvise |
|----------|---------|-----------|
| **macOS** | Full | Full |
| **iOS/iPad** | Full (remote) | Full (remote) |

Mobile connects to remote lfd server. Same experience, remote execution—interactive terminal streams to device.

## Server Evolution

### Current: Python lfd
- Unix socket for events
- HTTP for commands
- SQLite local

### Future: Rust lfd
- gRPC primary (control.proto)
- HTTP for health/status/metrics
- Same SQLite local, Postgres for hosted

## Client Strategy

Abstract the transport. Concerto should work with either backend.

```swift
protocol WaveService {
    func list(repo: URL) async throws -> [Wave]
    func create(wave: WaveConfig) async throws -> Wave
    func run(waveId: String, overrides: RunOverrides?) async throws
    func stop(waveId: String) async throws
    func connect(waveId: String) async throws -> InteractiveSession
}

// Implementations:
// - LocalWaveService (Unix socket + HTTP, current Python lfd)
// - GRPCWaveService (gRPC, future Rust lfd)
// - RemoteWaveService (HTTPS + auth, mobile to hosted)
```
