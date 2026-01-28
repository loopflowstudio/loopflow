# Protocol-First Engine Design

## Problem

Loopflow needs a stable API for multiple clients (lf CLI, Concerto, web dashboards) to control the daemon and engine. The current JSON-over-socket protocol works but lacks versioning, typed errors, and streaming primitives. A Rust rewrite of `lfd` requires this boundary to be well-defined before implementation begins.

The protocol is the contract. Get it right, and clients and servers can evolve independently. Get it wrong, and every change requires coordinated releases across Python, Rust, and Swift.

## Approach

Ship a two-tier protobuf schema with gRPC as primary transport and JSON-over-HTTP as fallback:

1. **Control plane** (`control.proto`) — Client-facing wave management, observability, event streaming
2. **Engine contract** (`engine.proto`) — Execution interface between daemon and core

The proto files are the source of truth. Code is generated for Python (grpcio-tools), Rust (tonic-build), and Swift (swift-protobuf). JSON fixtures validate schema compatibility across releases.

Start with the existing `proto/` directory structure and wire it into the Python daemon. Rust implementation comes later—the protocol ships first.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| JSON Schema + OpenAPI | Maximum compatibility, easy curl debugging | Weaker typing, no native streaming, harder to evolve safely |
| JSON-only with manual versioning | Simpler tooling, no protoc dependency | Schema drift is inevitable, no codegen across languages |
| Cap'n Proto / FlatBuffers | Zero-copy performance | Limited ecosystem, poor Swift support, overkill for this workload |
| Separate protocols per language | Each client gets native feel | Impossible to maintain, breaks when semantics diverge |

gRPC + Protobuf is the right choice because:
- Strong schema with cross-language codegen
- Native streaming for events and step output
- Versioning semantics baked into protobuf conventions
- Connect compatibility layer available for browsers later

## Key decisions

**Two-tier surface, not one.** Control plane (lf/Concerto → lfd) and engine contract (lfd → lf-core) are separate services. Control plane is public API; engine contract is internal. Different stability guarantees.

**Protobuf-first, JSON-compatible.** Primary transport is gRPC. Fallback is JSON-over-HTTP for debugging and simple tooling. Both use the same schema—proto3 field names are snake_case in JSON automatically.

**No WebSocket streaming.** Interactive steps use request/response: `ConnectWave` returns connection details, user runs step in terminal, `StepRunEnd` signals completion. Server-side streaming via `Subscribe` RPC handles event delivery. This avoids WebSocket complexity.

**Idempotency keys everywhere.** All mutating operations (`CreateWave`, `RunWave`, `StartStepRun`) accept idempotency keys. Clients can safely retry without duplicate effects.

**Typed errors with retry hints.** Every error includes machine-readable code, human message, retryability flag, and suggested retry delay. Observability via `trace_id`.

**Protocol version handshake.** Clients check `GetHealth` response for `protocol_version`. Refuse connection if major version differs. Warn if server minor is older than client.

## Scope

In scope:
- Finalize `control.proto` and `engine.proto` schemas
- Generate Python bindings and wire into existing daemon
- Generate Swift bindings for Concerto
- JSON compatibility layer for debugging
- Golden fixtures for compatibility testing
- Versioning documentation

Out of scope:
- Rust implementation of lfd (Stage 2)
- Authentication beyond API keys (JWT/OIDC is Stage 3)
- Multi-tenant routing (Stage 3)
- WebSocket or long-polling alternatives

## Done when

```bash
# Python daemon serves gRPC
grpcurl -plaintext localhost:50051 loopflow.control.v1.ControlService/GetHealth

# JSON fallback works
curl localhost:8080/v1/health

# Swift client compiles
swift build --target LoopflowProto

# Fixtures pass
pytest tests/test_proto_fixtures.py

# Version handshake enforced
# (client with protocol_version 2.0.0 refuses server at 1.0.0)
```

## Implementation phases

### Phase 1: Schema finalization
- Review `control.proto` and `engine.proto` for gaps
- Add missing fields identified in Python implementation audit
- Write comprehensive golden fixtures

### Phase 2: Python bindings
- Generate stubs with grpcio-tools
- Implement gRPC server alongside existing socket server
- Migrate socket handlers to call gRPC handlers internally
- JSON-over-HTTP adapter using grpc-gateway or custom middleware

### Phase 3: Swift bindings
- Generate Swift stubs with swift-protobuf
- Update Concerto client to use gRPC
- Fallback to JSON for development/debugging

### Phase 4: Deprecate socket protocol
- Mark socket protocol deprecated
- Update lf CLI to use gRPC
- Remove socket code after one release cycle

## Protocol surface summary

### Control plane (46 RPCs across 8 groups)

**Health:** GetStatus, GetHealth
**Waves:** ListWaves, GetWave, CreateWave, UpdateWave, DeleteWave, CloneWave, RunWave, StopWave, ConnectWave
**Flows:** ListFlows
**Worktrees:** ListWorktrees, NotifyWorktreeChanged
**Scheduler:** GetSchedulerStatus, AcquireSlot, ReleaseSlot
**StepRuns:** ListStepRuns, GetStepRunHistory, StartStepRun, EndStepRun
**Events:** Subscribe (streaming)
**Notifications:** Notify, StreamOutput

### Engine contract (14 RPCs across 6 groups)

**Context:** GatherContext, TrimContext, AnalyzeTokens
**Prompt:** FormatPrompt
**Steps:** RunStep (streaming), RunInteractiveStep
**Flows:** RunFlow (streaming), TickFlow, RunFork (streaming), Synthesize (streaming)
**Artifacts:** LoadStep, LoadFlow, LoadDirection
**Messages:** GenerateCommitMessage, GeneratePRMessage

### Events (14 types)

session.started, session.ended, output.line, worktree.updated, worktree.pruned, wave.created, wave.updated, wave.deleted, wave.started, wave.stopped, wave.activated, wave.waiting, scheduler.slot.acquired, scheduler.slot.released

## Open questions

None remaining. Previous questions resolved:
- Bi-directional streaming for interactive steps → No, use request/response with session events
- Strict protobuf-only for v1 → Yes, JSON is compatibility layer only, schema comes from proto

## Migration notes

The existing `proto/` directory has most of this already. The work is:
1. Audit Python implementation against proto schema for missing fields
2. Wire up gRPC server in Python daemon
3. Generate and test Swift bindings
4. Migrate socket protocol callers to gRPC
