# Orchestration expansion review (gate)

## What was implemented

- Added a unified activation ingress (`triggers/activation.rs`) used by watch/cron/loop/manual/listen stimuli.
- Added activation queue semantics: per-wave pending queue, stimulus-level coalescing, queue-cap drops, and dispatch through scheduler slots.
- Added activation audit storage (`activation_log`) plus run linkage (`wave_runs.activation_log_id`).
- Added push-trigger ingestion paths:
  - `POST /hooks/git` (local git hook)
  - `POST /v0/hooks/github` push webhook (signature-verified)
- Wired listen stimuli to enqueue target-wave activations when source waves complete.
- Added operator observability:
  - WS events: `activation_queued`, `activation_coalesced`, `activation_dropped`
  - HTTP endpoint: `GET /v0/waves/{wave_id}/activations`
- Hardened manual run UX: manual activation queue failures now return `500` instead of silently reporting `started: false`.

## Key choices

- Centralized trigger-to-run policy in one activation module instead of trigger-specific enqueue logic.
- Kept pollers running as reconciliation loops while adding push fast paths.
- Used constrained activation metadata (`source`, `reason`, SHA range) for diagnosability without introducing a dynamic expression engine.
- Persisted activation outcomes as immutable log entries (`queued/coalesced/dropped/dispatched`) for post-hoc analysis.

## How it fits together

Stimulus producers (watch poll, cron poll, loop ticker, manual run, listen completion, git/github hooks) call `enqueue_pending_activation(...)` with an `ActivationEnvelope`. The activation layer coalesces/drops/queues and records an activation log + WS event. The activation dispatcher drains pending entries when scheduler capacity is available, creates a wave run, links it to the dispatch log, and starts execution through existing slot-guarded run spawning.

## Risks and bottlenecks

- Queue-cap is currently a fixed default (`20`) rather than per-wave configurable; high-churn waves may drop more activations than desired.
- Dispatcher is interval-driven (1s); latency is good but still not instant under sustained load.
- Local `xcodebuild` UI test run failed due `ConcertoUITests-Runner` bootstrap crash (environment/runner issue), while Rust/Python/Swift package tests and e2e smoke passed.

## What's not included

- Milestone B flow-language enrichment is still out of scope here (`when` predicates, multi-step fork branch plans, persisted flow-branch decisions).
- No scheduler-model replacement (existing slot discipline retained).
- No expression engine for arbitrary trigger conditions.
