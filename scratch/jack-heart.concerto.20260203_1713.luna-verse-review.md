# Gate Review — lfd mobile prep

## What was implemented
- Added HTTP REST and WebSocket endpoints to lfd for wave CRUD, actions, and live event streaming.
- Introduced shared EventHub broadcasting to power both gRPC subscriptions and WebSocket clients.
- Added loopflow.studio registration + heartbeat, connection-token validation, and HTTP/gRPC auth checks.
- Hooked git hooks to POST localhost HTTP events instead of the UNIX socket.
- Added machine identity, config/credentials loading, and wiring in main startup.

## Key choices
- WebSocket uses query `?token=` or Authorization headers for auth; localhost bypasses auth.
- HTTP binds to `0.0.0.0:2486` and requires registration for non-loopback access.
- Event payloads are normalized to JSON with a `type` plus `data` payload.
- Registration failure keeps local access working while blocking remote access.

## How it fits together
- `main.rs` wires `EventHub`, `AuthContext`, and `RegistrationClient` into both gRPC (`ControlServer`) and HTTP (`HttpState`).
- Event emissions occur in gRPC handlers and HTTP endpoints, then flow through `EventHub` to WebSocket and gRPC subscribers.
- HTTP endpoints use store access via `spawn_blocking` wrappers to reuse existing `RunStore` methods.

## Risks and bottlenecks
- Remote access depends on loopflow.studio registration + connection-token validation; a mismatch in mobile auth expectations would block clients.
- HTTP event payloads are JSON serialized from proto structures; any proto changes need mirrored JSON mapping updates.
- `run_wave_handler`/`land_wave_handler` spawn blocking tasks; heavy load could saturate the blocking thread pool.

## What's not included
- GitHub polling for PR/CI status.
- TLS certificate management for remote HTTPS/WSS.
- Output streaming over HTTP (reserved via `OutputHub` but not wired).
