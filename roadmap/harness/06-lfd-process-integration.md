# 06: lfd Process Integration

Wire `lfd` chat endpoints to spawn `lf-agent`, stream JSONL events, and persist turn state.

## What exists after this

- `/v0/waves/:id/chat/*` endpoints backed by real process execution
- live streaming of progress/final/tool events
- chat lane executes alongside wave runs
- per-turn workspace snapshot captured at turn start

## Commit slices

### C1 — Endpoint + orchestration wiring (~300-500 LOC)

- `POST /chat/messages` spawn path
- `GET /chat`, `PATCH /chat/memory`, `POST /chat/compact`, `DELETE /chat`
- lane-aware execution state

### C2 — JSONL event ingestion + persistence (~300-550 LOC)

- parse agent stdout JSONL
- persist messages/memory edits/tool traces
- surface progress/final stream to clients

### C3 — Concurrency + snapshot rules (~250-450 LOC)

- chat lane independent from step lane
- bind turn to branch/head sha snapshot at start
- tests for lane behavior + persistence ordering

## Constraints

- Match executor type (container/process) for chat lane.
- Keep event handling resilient to partial agent failure.
- Keep contract compatibility with Foundation Contract.

## Done when

```bash
cargo test -p loopflow lfd_chat
```

Expected: endpoint/orchestration tests pass.
