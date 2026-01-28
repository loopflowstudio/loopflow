# Rust Protocol-First Engine (Stage 1)

## Problem

Project 1 must define a protocol that Rust and Python (and eventually Swift) can all speak, then use that as the foundation for implementing the Rust control plane and engine. Today the API surface is implicit and tangled in Python internals, which blocks a shared, durable protocol for remote clients, managed clusters, and long-lived reliability.

## Approach

Define a single, schema-first protocol as the primary design artifact, shared across Rust, Python, and eventually Swift, then derive all client/server surfaces from it.

- **Protocol-first contract:** A versioned schema that covers control plane requests, execution engine calls, and event streaming.
- **Two-tier surface:**
  - **Control plane API** (`lf`/Concerto → `lfd`): run management, flow lifecycle, observability, authn/z, multi-tenant routing.
  - **Engine contract** (`lfd` ↔ `lf-core`): execution, context assembly, prompt formatting, token limits, artifact I/O.
- **Transport strategy:** gRPC + Protobuf as the default; optional JSON/HTTP compatibility for simple tooling.
- **Compatibility policy:** semantic protocol versioning, explicit ranges, and a client refuse-incompatible rule.
- **Error model:** typed errors with retry hints, idempotency keys, and trace IDs.

Deliverables for Stage 1 are the protocol spec (proto files + JSON schema companion), compatibility rules, and a golden set of event payload fixtures that future implementations must match.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| REST + JSON Schema only | Maximum tooling compatibility, easy curl debugging | We lose strict typing and streaming semantics needed for run events and mobile/desktop codegen. |
| Twirp or Thrift | Simpler deployment and mature IDLs | We need streaming and modern ecosystem support for multi-platform clients. |
| gRPC only, no JSON fallback | Strong typing and streaming | Debuggability and lightweight tooling suffer; optional JSON adds pragmatic flexibility. |
| Define protocol after Rust rewrite | Faster short-term coding | Locks in implicit contracts and makes remote/managed mode brittle. |

## Key decisions

- **One canonical schema** (proto) drives all generated types and compatibility tests.
- **Separate control vs engine contracts** so `lf` can switch between local engine and remote daemon without UX drift.
- **Streaming events are first-class**; event payloads are versioned and tested with golden fixtures.
- **Compatibility is enforced, not advisory**; incompatible clients must refuse to connect.

## Scope

- In scope:
  - Protobuf schema for control + engine APIs
  - Versioning and compatibility rules
  - Error codes, retry semantics, idempotency keys
  - Event stream schema + golden fixtures
  - Authn/z hooks in protocol surface (API key + JWT stubs)
  - Explicit UX invariants: default `lf` workflow and behavior remain unchanged
- Out of scope:
  - Implementing Rust server/runtime
  - Migrating existing Python internals
  - New UX flows or prompt changes

## Done when

- `proto/loopflow/control/v1` and `proto/loopflow/engine/v1` schemas exist with documented versioning rules.
- Golden event payload fixtures pass a schema-compatibility test suite.
- A lightweight client can validate a compatible server and refuse incompatible versions without custom logic.
