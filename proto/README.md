# Loopflow Protocol

Protocol definitions for loopflow's control plane and execution engine.

## Structure

```
proto/
├── README.md               # This file
├── VERSIONING.md           # Compatibility rules
├── loopflow/
│   ├── control/v1/         # Control plane API (lf/Concerto → lfd)
│   │   └── control.proto
│   └── engine/v1/          # Engine contract (lfd ↔ lf-core)
│       └── engine.proto
└── fixtures/               # Golden test fixtures
    └── events/
```

## Two-Tier Surface

### Control Plane (`control/v1`)

Client-facing API for wave management, observability, and event streaming.

**Clients:** `lf` CLI, Concerto (Swift), web dashboards

**Covers:**
- Wave CRUD and lifecycle
- Flow discovery
- Worktree management
- Scheduler slots
- StepRun tracking
- Event subscriptions

### Engine Contract (`engine/v1`)

Execution interface between daemon and core engine.

**Clients:** `lfd` daemon

**Covers:**
- Context assembly
- Prompt formatting
- Step/flow execution
- Fork/synthesize patterns
- Message generation

## Transport

**Primary:** gRPC + Protobuf for typed streaming

**Fallback:** JSON-over-HTTP for simple tooling

Both transports use the same schema. JSON field names use snake_case per proto3 conventions.

## Quick Reference

### Control Plane Methods

| Method | Description |
|--------|-------------|
| `GetStatus` | Daemon health summary |
| `GetHealth` | Detailed health + protocol version |
| `ListWaves` | List waves for a repo |
| `CreateWave` | Create new wave |
| `UpdateWave` | Modify wave configuration |
| `RunWave` | Start wave execution |
| `StopWave` | Stop running wave |
| `Subscribe` | Stream events matching patterns |

### Engine Methods

| Method | Description |
|--------|-------------|
| `GatherContext` | Assemble prompt components |
| `TrimContext` | Fit context to token budget |
| `FormatPrompt` | Build final prompt string |
| `RunStep` | Execute single step (streaming) |
| `RunFlow` | Execute flow (streaming) |
| `RunFork` | Parallel execution with directions |
| `Synthesize` | Combine fork results |

## Events

Control plane events for real-time updates:

| Event | Description |
|-------|-------------|
| `wave.created` | New wave added |
| `wave.started` | Wave execution began |
| `wave.stopped` | Wave execution ended |
| `session.started` | StepRun began |
| `session.ended` | StepRun completed |
| `output.line` | Streaming output |
| `worktree.updated` | Branch/PR state changed |

Subscribe with glob patterns: `wave.*`, `session.*`, `worktree.updated`

## Generating Code

### Python (grpcio-tools)

```bash
python -m grpc_tools.protoc \
  -I proto \
  --python_out=src/loopflow/proto \
  --grpc_python_out=src/loopflow/proto \
  proto/loopflow/control/v1/control.proto \
  proto/loopflow/engine/v1/engine.proto
```

### Rust (tonic-build)

```rust
// build.rs
tonic_build::configure()
    .build_server(true)
    .build_client(true)
    .compile(
        &["proto/loopflow/control/v1/control.proto",
          "proto/loopflow/engine/v1/engine.proto"],
        &["proto"],
    )?;
```

### Swift (swift-protobuf)

```bash
protoc \
  --swift_out=swift/Sources/LoopflowProto \
  --grpc-swift_out=swift/Sources/LoopflowProto \
  proto/loopflow/control/v1/control.proto \
  proto/loopflow/engine/v1/engine.proto
```

## See Also

- [VERSIONING.md](VERSIONING.md) — Compatibility rules
- [fixtures/](fixtures/) — Golden test fixtures
