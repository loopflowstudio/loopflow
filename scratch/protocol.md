# Protocol-First Engine Design

## Summary

Loopflow standardizes daemon/client integration on a two-tier protobuf schema with gRPC as the primary transport and JSON-over-HTTP as a compatibility layer. The proto files are the source of truth; JSON and generated code follow them.

## Current state

- Control-plane gRPC server is implemented alongside the existing socket server.
- JSON-over-HTTP v1 endpoints mirror proto responses for debugging and compatibility.
- Python protobuf/gRPC bindings are generated and checked in; fixtures/tests validate schema drift.
- Protocol version handshake is exposed via health checks.

## Decisions

- **Two-tier surface.** Control plane (lf/Concerto → lfd) is public; engine contract (lfd → lf-core) is internal.
- **Protobuf-first.** gRPC is primary; JSON is a compatibility layer derived from proto schemas.
- **No WebSocket streaming.** Use server-side streaming via `Subscribe` and request/response for interactive steps.
- **Idempotency keys on mutations.** Safe retries for create/run/start operations.
- **Typed errors with retry hints.** Errors include machine code, human message, retryability, and delay.

## Scope

In scope:
- Control plane and engine proto schemas
- gRPC server for control plane
- JSON compatibility endpoints
- Fixtures and schema-compatibility tests
- Versioning documentation

Out of scope:
- Rust daemon implementation
- Full engine contract gRPC execution/streaming
- Auth beyond API keys

## Remaining gaps

- Engine contract gRPC implementation (streaming execution not wired)
- Some control-plane RPCs may still be partial across gRPC/HTTP parity
- Swift client integration still pending

## Risks

- Large API surface can lead to partial implementations and client gaps.
- JSON and gRPC behavior must remain aligned to avoid client divergence.
- Checked-in generated code can drift without consistent regeneration.

## Migration notes

1. Audit Python implementation against proto schema for any missing fields.
2. Expand control-plane RPC parity across gRPC and HTTP v1 as needed.
3. Generate and validate Swift bindings for Concerto.
4. Migrate lf CLI to gRPC once parity is acceptable.

