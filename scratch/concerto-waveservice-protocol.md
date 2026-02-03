---
status: todo
phase: 2
---

# WaveService Protocol Abstraction

Abstract the transport layer so Concerto works with both Python lfd and future Rust lfd.

## Current

`LFDClient.swift` talks directly to Python lfd via Unix socket + HTTP.

## Build

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

## Done when

Same UI code works against both Python and Rust lfd backends.
