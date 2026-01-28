# Protocol-First Engine (Stage 1)

## Problem

Loopflow needs a stable, versioned protocol that works for local and remote control without changing UX. Today the daemon boundary is implicit; moving to Rust + remote clients demands a clear contract for runs, flows, events, and interactive session connect, plus error semantics and compatibility rules that can survive multi-tenant and enterprise use.

## Approach

Define a protocol-first surface in `proto/` that separates **control plane APIs** from the **engine contract** used by `lf` in local mode. Make the protocol the primary artifact; code and clients derive from it. Use gRPC + Protobuf for core APIs, with an optional Connect/JSON bridge for browser/mobile and simple tooling. Standardize event streaming, version negotiation, and a typed error model with retry hints and idempotency keys.

Implementation plan (stage 1 deliverables only):
- **Schemas:** Add `proto/loopflow/control/v1/*.proto` and `proto/loopflow/engine/v1/*.proto` with the v1 API surface from `roadmap/rust/01-protocol.md`.
- **Versioning:** Encode semantic protocol version in every response plus a handshake response that advertises supported ranges.
- **Errors:** Define `Error` and `ErrorCode` enums, `retryable`, `idempotency_key`, and `trace_id` fields.
- **Events:** Standardize `Event` shape and streaming semantics for run progress and session lifecycle.
- **Session connect:** Include `ConnectWave`, `StepRunStart`, `StepRunEnd` and the associated `wave.waiting/session.started/session.ended` events.
- **Docs + tests:** Add a `proto/README.md` describing compatibility rules and a small golden-payload test suite that validates schema changes.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| JSON Schema + REST | Easy tooling, curl-friendly | Weaker typing, harder evolution, ad-hoc streaming |
| Twirp | Simple HTTP/1.1 | No native streaming; less aligned with multi-client needs |
| Thrift | Mature IDL | Tooling less common in modern mobile/desktop stacks |
| Protobuf only (no JSON bridge) | Strong typing, single path | Harder for browser/mobile and simple tools |
| Single unified API (no engine contract) | Fewer schemas | Blurs local vs remote boundary; makes engine swap harder |

## Key decisions

- **gRPC + Protobuf as the default.** It provides strong typing, stable evolution, and first-class streaming for event timelines.
- **Two surfaces:** `control` APIs (lfd-facing) and `engine` APIs (lf-facing). The engine contract is the minimal API `lf` needs and is the portability boundary.
- **Versioning is explicit.** Every response carries protocol version plus supported range in handshake to prevent silent drift.
- **Typed errors with retry semantics.** Clients can implement deterministic backoff and idempotent retries.
- **Session connect stays request/response.** Avoids WebSocket complexity while keeping Concerto + CLI parity.

## Scope

- In scope: proto schemas, versioning rules, error model, streaming/event shapes, session connect contract, docs + golden tests.
- Out of scope: server runtime, daemon scheduling, auth implementation, client codegen, Python internals.

## Done when

- `proto/loopflow/control/v1/` and `proto/loopflow/engine/v1/` schemas exist and cover runs, flows, events, and session connect.
- `proto/README.md` documents versioning rules and compatibility guarantees.
- A small golden payload suite validates event and error shapes for v1.
