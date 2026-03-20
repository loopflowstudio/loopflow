---
linear_id: cf42f199-4cab-4f57-9691-be0704856c6a
---
# Attention Queue Completion

**Finish line:** Design review and calibration checkpoints surface as typed attention items with queue-specific detail and actions, so the attention queue covers every human decision in build and tend flows.

## Context

The attention queue foundation now exists: `AttentionItem` storage and APIs in `lfd`, websocket updates, and a macOS queue home screen that already handles code review, queue failures, and step failures.

The remaining gap is coverage. `design_review` and `calibration` are modeled in Rust and Swift, but they still fall back to raw context and never get created by the executor or tend flow. Until those two paths are real, the queue cannot fully replace drilling into individual waves.

## What to build

1. **Design review attention creation.** Wire `review-design` / `kickoff` outputs into `design_review` attention items with stable IDs, typed context, and resolution rules tied to the wave advancing or being redirected.
2. **Calibration attention creation.** Wire `tend/draft-chord` into `calibration` attention items that capture assessment summary, proposed mutations, and any human notes that should feed later tend cycles.
3. **Typed queue detail and actions.** Replace the current raw JSON fallback for `design_review` and `calibration` with dedicated Swift decoding, filters, detail layouts, and action buttons that call the right domain APIs.
4. **Lifecycle and urgency polish.** Ensure new attention kinds participate in urgency sorting, viewed/resolved transitions, history, and websocket updates the same way queue failures and code reviews do.
5. **Proof through tests.** Add Rust and Swift coverage that shows these items are created, rendered, and resolved end to end rather than just modeled in enums.

## Done when

* `review-design` or `kickoff` produces `design_review` attention items with typed context and actionable queue detail
* `tend/draft-chord` produces `calibration` attention items with mutation context and review actions
* The queue UI exposes calibration distinctly and no modeled attention kind falls back to `.raw` JSON in normal use
* Reconciliation resolves design-review and calibration items when the human action clears the underlying condition
* A conductor can handle build and tend checkpoints from the queue without opening a wave detail first
