# Design Review: Protocol-First Schema for Rust Port

## What was implemented

Protocol-first schema defining the contract between loopflow clients and servers, designed to support the eventual Rust port while remaining immediately usable for Python implementations.

**Deliverables:**
1. `proto/loopflow/control/v1/control.proto` — Control plane API (624 lines)
2. `proto/loopflow/engine/v1/engine.proto` — Engine contract (540 lines)
3. `proto/README.md` — Protocol documentation with codegen examples
4. `proto/VERSIONING.md` — Compatibility rules and migration guidance
5. Golden fixtures for 11 event types, 2 requests, 7 responses
6. `tests/test_proto_fixtures.py` — 59 tests validating fixtures against schema

## Key choices

**Two-tier surface.** Control plane (wave management, observability, event streaming) is separate from engine contract (context assembly, step/flow execution). This allows `lf` to switch between local engine and remote daemon without UX drift.

**Proto3 with JSON fixtures.** The `.proto` files are the source of truth; JSON fixtures are human-readable validation data. This supports both gRPC and JSON/HTTP transports.

**Semantic versioning with refuse-incompatible rule.** Clients must check `protocol_version` in `GetHealth` and refuse to connect if major version differs. This prevents silent compatibility failures.

**Idempotency keys on mutating operations.** All Create/Update/Run requests include `idempotency_key` for safe retries and deduplication.

**Events as first-class schema objects.** Each event type has a dedicated message type and golden fixture. This ensures event payloads are versioned and testable.

## How it fits together

```
┌─────────────────────────────────────────────────────────────┐
│  Clients (lf CLI, Concerto, web dashboards)                 │
└─────────────────────┬───────────────────────────────────────┘
                      │ ControlService (gRPC/JSON)
                      ▼
┌─────────────────────────────────────────────────────────────┐
│  lfd daemon                                                 │
│  - Wave CRUD, lifecycle                                     │
│  - Scheduler slots                                          │
│  - Event streaming                                          │
└─────────────────────┬───────────────────────────────────────┘
                      │ EngineService (gRPC/JSON)
                      ▼
┌─────────────────────────────────────────────────────────────┐
│  lf-core engine                                             │
│  - Context assembly (GatherContext, TrimContext)            │
│  - Prompt formatting                                        │
│  - Step/flow execution                                      │
│  - Fork/synthesize patterns                                 │
└─────────────────────────────────────────────────────────────┘
```

The proto files cover the full surface between these layers. JSON fixtures validate that event payloads can round-trip through implementations.

## Risks and bottlenecks

**Proto compilation not yet integrated.** The `.proto` files exist but codegen for Python/Rust/Swift isn't wired into the build. Future work will add `grpcio-tools`, `tonic-build`, and `swift-protobuf` integration.

**No runtime validation yet.** The current implementation doesn't parse protos at runtime—it validates JSON fixtures against hardcoded enum sets. A future step could use `protobuf` to parse and validate at runtime.

**Event fixture coverage.** Some event types defined in the proto don't have fixtures yet (e.g., `wave.updated`, `wave.deleted`). These can be added incrementally.

## What's not included

- Rust server implementation (Stage 2)
- Python migration to generated types (Stage 3)
- Swift client codegen (Stage 4)
- New UX flows or prompt changes
- Authentication/authorization implementation (stubs in protocol only)

## Test coverage

59 tests in `tests/test_proto_fixtures.py`:
- 33 parameterized tests across event fixtures
- 5 request fixture tests
- 11 response fixture tests
- 4 cross-fixture consistency tests
- 6 proto file/doc existence tests

All 663 tests in the suite pass.
