# Design Review: WaveService Protocol Abstraction + Phase 1 Complete

## What was implemented

1. **WaveServiceProtocol** - Transport-agnostic protocol for all wave operations in LoopflowCore
2. **EventServiceProtocol** - Separate protocol for event streaming (different lifecycle)
3. **LocalWaveService** - HTTP implementation conforming to the protocol (renamed from WaveService)
4. **LocalEventService** - Socket implementation conforming to EventServiceProtocol (renamed from LFDEventService)
5. **WaveServiceFactory** - Context-based factory returning appropriate implementation
6. **WaveRuns HTTP endpoint** - New `/wave-runs` endpoint replacing direct SQLite reads
7. **Roadmap update** - Phase 1 marked complete, Phase 2 now active

## Key choices

| Decision | Why |
|----------|-----|
| Separate EventServiceProtocol | Event streaming has persistent connection lifecycle, doesn't fit request/response pattern |
| Factory returns `any WaveServiceProtocol` | Swift 6 existential requirement; callers don't need concrete type |
| Delete LFDClient.swift | Redundant with LocalWaveService; `checkAvailability()` now on protocol |
| Move WaveRuns behind HTTP | Direct SQLite reads bypass daemon, blocks remote access |
| Parameter naming: `(_ id:)` vs `(waveId:)` | Protocol uses concise `id` parameter; matches Swift API guidelines |

## How it fits together

```
RepoState.swift
    │
    ├── waveService: LocalWaveService (conforms to WaveServiceProtocol)
    │                           │
    │                           └── HTTP → lfd daemon
    │
    └── eventService: LocalEventService (conforms to EventServiceProtocol)
                                │
                                └── Unix socket → lfd daemon
```

Transport details live in the service implementations. RepoState imports protocol-conforming types from LoopflowCore. The factory exists for future gRPC/remote implementations.

## Risks and bottlenecks

| Risk | Mitigation |
|------|------------|
| Factory always returns LocalWaveService | Intentional for Phase 1; grpc/remote cases documented as future work |
| WaveRuns HTTP adds network hop | Negligible latency; required for remote access |
| New `parseWaveRunFromJSON` duplicates some parsing | Consolidates parsing in one place; cleaner than SQLite column mapping |

## What's not included

- **GRPCWaveService** - Phase 2 follow-up (grpc-swift dependency)
- **RemoteWaveService** - Phase 3 (authentication required)
- **v1 endpoint for /wave-runs** - Added question to scratch/questions.md
- **Config-driven factory selection** - Added question to scratch/questions.md

## Test results

- Python tests: 679 passed, 1 skipped
- Swift tests: 81 passed
- Swift build: clean

## Files changed

| File | Change |
|------|--------|
| `WaveServiceProtocol.swift` | New - protocol + supporting types |
| `WaveServiceFactory.swift` | New - factory with context enum |
| `LocalWaveService.swift` | Renamed from WaveService, conforms to protocol |
| `LocalEventService.swift` | Renamed from LFDEventService, conforms to protocol |
| `WorktreeService.swift` | Inlined LFDClient's worktree listing (was only user) |
| `LFDClient.swift` | Deleted - redundant |
| `FlowRun.swift` → `WaveRun.swift` | Renamed model to match API naming |
| `RepoState.swift` | Uses `LocalWaveService` (protocol-conforming) |
| `WaitingStateCard.swift` | Uses `LocalWaveService` (protocol-conforming) |
| `http_server.py` | Added `/wave-runs` endpoint |
| `lfd/README.md` | Documented new endpoint |
| `roadmap/concerto/README.md` | Phase 1 complete, Phase 2 in progress |
