# Unified Session API + Codex Adapter

Standalone session management for `lfd` with durable event replay and a Codex app-server adapter.

## Current state

Implemented in this branch:

- Session lifecycle manager with `starting → active → ending → ended|failed`.
- `/v0/sessions` HTTP endpoints:
  - `POST /v0/sessions`
  - `GET /v0/sessions/{id}`
  - `POST /v0/sessions/{id}/input`
  - `GET /v0/sessions/{id}/events` (SSE replay + live follow)
  - `DELETE /v0/sessions/{id}`
- Storage migration `010_sessions.sql` with append-only `session_events`.
- Sqlite + Postgres store methods for sessions and session event persistence/replay.
- Codex adapter (`codex --app-server`) with JSON-RPC notification mapping into flat `SessionEvent`s.
- Manager tests for lifecycle behavior, provider gating, and one-active-session-per-wave-run enforcement.
- API docs in `docs/lfd.md` with curl examples.

## API behavior

### Session create

`POST /v0/sessions` returns immediately with `status: "starting"` and transitions to `active` asynchronously after adapter startup completes.

### Session input

`POST /v0/sessions/{id}/input` is valid only when the session is `active`. The codex adapter forwards input as `turn/start` or `turn/steer`.

### Session events

`GET /v0/sessions/{id}/events` replays persisted events from storage, then follows live events over SSE. Clients can pass `after_seq` to skip older replay items.

### Session end

`DELETE /v0/sessions/{id}` is idempotent and performs graceful shutdown (`turn/interrupt` when needed, then process stop).

## Core model and architecture

- Flat typed event stream (`SessionEvent`) with serde-tagged JSON payloads.
- `SessionManager` owns lifecycle transitions, persistence, and live broadcast.
- Replay path is store-backed (`session_events`); live tail is broadcast-backed.
- At most one active session per `wave_run_id`.
- Current provider scope is intentionally codex-only.

## Known risks and follow-ups

- Codex JSON-RPC payload assumptions were inferred and should be validated against real codex traces.
- Confirm asynchronous create semantics (`starting` on create, then `active`) as final API contract.
- Active runtimes are process-local; restart rehydration is not implemented.
- SSE lagged receivers currently skip missed live messages instead of in-stream store backfill.

## Out of scope in this iteration

- Wave orchestration beyond optional `wave_run_id` metadata.
- Additional providers beyond codex.
- Resume/rehydration of active adapter processes after daemon restart.
