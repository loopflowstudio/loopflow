# Wave Status API

**Finish line:** `lfd` HTTP API returns real-time wave status — mode, current step, status enum, beat history — for all active waves. One GET endpoint that gives a client everything it needs to render the runboard.

## Context

lfd already manages wave state and serves HTTP. The runboard needs a dedicated endpoint that assembles the wave record (name, mode, step, status, provider, branch, worktree) and recent beat history into a single response. This is the data layer that both Concerto and a web UI would consume.

The status enum: `running` (agent executing), `idle` (loop wave between beats), `blocked` (waiting on human review or CI), `error` (step failed), `done` (manual wave completed), `sleeping` (cron wave between runs).

## What to build

- GET endpoint returning all waves with their current status, mode, step, and recent beats
- Status enum derived from lfd's existing run/session state
- Beat history from run journals (play/tune/silence, timestamp, outcome)
- WebSocket or SSE stream for real-time updates (so the UI doesn't poll)

## What to skip

- Agent-specific health detection (item 3 handles that)
- Any UI rendering — this is the API only
- Shared scratchpad (Phase 2)
