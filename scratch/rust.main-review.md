# rust.main review

## What was implemented
- Added a gRPC control-plane server alongside the existing socket server, with health/status, scheduler, step run, and worktree-related RPCs.
- Added a JSON-over-HTTP v1 compatibility layer that mirrors proto responses and endpoints.
- Introduced generated protobuf/gRPC Python bindings, a generation script, and fixtures/tests for protocol compatibility.
- Wired HTTP/gRPC servers into daemon startup and added flow-run tracking helpers.

## Key choices
- gRPC is the primary transport while HTTP v1 is a compatibility/debug layer, keeping schema consistency via shared proto types.
- Protocol version handshake and health checks expose schema/version/metrics for client compatibility checks.
- Generated code is checked in for reliability and reproducible builds; regeneration is centralized in `loopflow.proto.generate`.

## How it fits together
- `grpc_server.py` implements `ControlService` directly on top of existing daemon logic and event broadcast hooks.
- `http_server.py` exposes both legacy REST endpoints and `/v1/*` endpoints aligned with proto schemas, using shared mapping helpers from `protocol_v1.py`.
- Proto fixtures and tests validate JSON/proto compatibility and guard schema drift.

## Risks and bottlenecks
- API surface area is large; missing or partial RPC implementations could surface as client gaps until engine/control coverage is complete.
- Generated protobuf files are committed; any local regeneration must be kept in sync to avoid drift.
- HTTP v1 and gRPC must remain behaviorally aligned; regressions could split client behavior.

## What's not included
- Full engine contract gRPC implementation and streaming execution.
- Scheduler/event streaming parity on HTTP v1 beyond the endpoints implemented here.
- Rust daemon implementation; this is Python-side protocol delivery only.
