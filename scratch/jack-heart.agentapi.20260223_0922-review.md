# Session API + Codex adapter review

## What was implemented

- Added standalone session management in `lfd` with lifecycle/status tracking (`starting → active → ending → ended|failed`).
- Added `/v0/sessions` HTTP routes:
  - `POST /v0/sessions`
  - `GET /v0/sessions/{id}`
  - `POST /v0/sessions/{id}/input`
  - `GET /v0/sessions/{id}/events` (SSE replay + live follow)
  - `DELETE /v0/sessions/{id}`
- Added durable storage for sessions and append-only session events via migration `010_sessions.sql` and store methods for sqlite + postgres.
- Added Codex app-server adapter (`codex --app-server`) that maps JSON-RPC notifications into flat `SessionEvent`s.
- Added manager tests for lifecycle, unsupported provider rejection, and single-active-session-per-wave-run enforcement.
- Added docs for the session API in `docs/lfd.md` with concrete curl examples.

## Key choices

- **Flat event model**: all adapter output is normalized into one stream of typed `SessionEvent`s for SSE and persistence.
- **Asynchronous startup**: `POST /v0/sessions` returns immediately in `starting`, then transitions to `active` once adapter startup completes.
- **Store-backed replay, broadcast-backed live tail**: replay always comes from `session_events`; live updates come from in-memory broadcast.
- **Codex-only provider gate (current scope)**: manager explicitly rejects non-`codex` providers for now.
- **Shutdown hardening added in gate pass**:
  - suppresses `codex_disconnected` error during intentional shutdown
  - cleans up adapter process/tasks if startup fails partway through

## How it fits together

`SessionManager` owns lifecycle/state transitions and persistence. Adapters emit `SessionEvent`s into a broadcast channel; manager persists each event with a per-session sequence and rebroadcasts persisted events to live subscribers. The HTTP layer is thin: routes call manager methods and SSE streams replay persisted events (optionally from `after_seq`) before following live events.

## Risks and bottlenecks

- **Codex JSON-RPC schema assumptions**: some request/notification payload shapes are inferred and may need adjustment against real traces.
- **In-memory runtime state**: live session runtimes are process-local; restart behavior for active sessions is not implemented yet.
- **SSE lag handling**: lagged broadcast receivers currently skip missed live messages rather than backfilling from storage in-stream.
- **Single-provider scope**: only `codex` is accepted today.

## What's not included

- No wave orchestration integration beyond optional `wave_run_id` metadata.
- No multi-provider adapter support beyond codex.
- No resume/rehydration of active adapter processes across daemon restarts.
- No auth/provider-session handshake enrichment beyond current event/status surface.
