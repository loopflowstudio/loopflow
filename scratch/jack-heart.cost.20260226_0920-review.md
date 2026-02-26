# Gate review — jack-heart.cost.20260226_0920

## What was implemented

- Added two new persisted session event payloads in Rust session types:
  - `turn_usage` (`TurnUsage`) for per-turn token/model/cost metadata.
  - `context_snapshot` (`ContextSnapshot`) for prompt composition at session start.
- Wired usage extraction/emission across all harness mappings:
  - Claude: usage parsed from `result` payload and emitted after `turn_completed`.
  - Codex: usage parsed from `turn/completed` params and emitted after `turn_completed`.
  - OpenCode: usage parsed from `session.status` transition (`active -> idle`) when usage data is present.
- Updated session startup flow to capture prompt breakdown and emit `context_snapshot` immediately after `status_changed(starting)`.
- Expanded conformance/unit tests for new event types, usage parsing, and serde round-trips.
- Added daemon docs for new session stream events in `docs/lfd.md`.

## Key choices

- **No DB migration:** kept storage schema unchanged by persisting new event variants inside existing `session_events.data` JSON.
- **Normalized event order:** `turn_usage` is emitted after `turn_completed`; `context_snapshot` is emitted once before the first turn.
- **Provider-tolerant parsing:** optional fields are tolerated (`Option<_>`), while core token counts default to `0` when absent.

Alternatives considered implicitly by implementation:
- Separate usage tables/materialization were deferred (kept event-log first for phase-01 scope).
- Provider-specific event shapes were not exposed directly; mappings normalize into one cross-provider payload.

## How it fits together

`prepare_launch_prompt` already returns `ContextBreakdown`; `SessionManager::prepare_session_prompt` now threads that breakdown through `create_session`, where it is converted to `ContextSnapshot` and persisted as a session event. During runtime, harness adapters map provider-native completion payloads into normalized `TurnUsage` and emit them through the existing event bridge/store pipeline. Consumers of `/v0/sessions/{id}/events` get replay + live updates with the new event types automatically.

## Risks and bottlenecks

- **Provider schema drift:** missing/renamed usage keys can silently degrade into `0`/`None`; metrics can look complete but be partially inferred.
- **OpenCode usage dependency:** usage only emits when `session.status` idle payload includes a `usage` block.
- **Aggregation scaling (future phases):** usage APIs built over raw event scans may need materialization if event volume grows.

## What's not included

- No usage aggregation endpoints (`/sessions/{id}/usage`, `/usage/summary`, etc.).
- No provider/model catalog elevation work.
- No Concerto inline usage UI or analytics dashboard.
- No `lfq usage` CLI views.
