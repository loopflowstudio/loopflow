# Attention queue review

## What was implemented
- Added a first-class `AttentionItem` model, storage, HTTP routes, and websocket event types in `lfd`.
- Replaced Concerto's default empty detail state with an attention queue that shows unresolved review/failure items and renders per-kind detail/actions.
- Added backend/store plumbing for queue failures, code review items, and step failure items, plus Swift parsing/state for attention items.
- Polished queue reconciliation so queue-failure items keep their original age/viewed state across repeated polls and now emit `attention_created` / `attention_updated` / `attention_resolved` events from poll, webhook, and run-completion paths.

## Key choices
- Kept `AttentionItem` as a projection of domain state rather than adding a separate decision API; the UI still acts through wave/domain endpoints.
- Reused deterministic queue-failure IDs per run so repeated queue reconciliation updates the same record instead of creating churn.
- Preserved `surfaced_at` / `viewed_at` for still-open queue failures so urgency ordering reflects how long the issue has existed, not how recently the reconciler ran.
- Updated `swift/README.md` with the new queue-first repo window behavior so the user-facing navigation shift is documented.

## How it fits together
`lfd` now stores unresolved human-attention state in `attention_items` and exposes it through `/v0/attention` plus websocket events. Queue reconciliation, run completion, and failed steps create or resolve attention items; Concerto's `AttentionStore` consumes those events and drives the new default `AttentionQueueView` when no wave is selected.

## Risks and bottlenecks
- Code review and step-failure items still resolve on the periodic attention reconciler, so they can linger until the next poll if no other refresh happens.
- `GET /v0/attention?repo=...` still filters by loading waves one-by-one, so very large repos could make the list call more expensive than necessary.
- The local macOS Xcode scheme still has a UI-test bootstrap issue: on March 17, 2026, `ConcertoUITests-Runner` exited early twice before establishing the test connection even though the scheme's unit tests passed.

## What's not included
- Automatic creation hooks for `design_review` and `calibration` attention items are still modeled but not wired up.
- The migration still creates the new `attention_items` table without copying historical `wave_queue_blocks` rows; current queue failures repopulate on reconciliation.
