---
linear_id: 23581d6d-b363-463a-936d-f7e29efbcac7
notion_id: 32af8f99-3d81-81f8-87ac-decf31e198c2
---
# Calibration view

**Needs:** workflows/3-wave-scheduling, workflows/4-letta-integration

**Finish line:** The garden flow's human checkpoint has a dedicated UX. Not a notification, not a PR review — a trajectory review across all waves. The highest-leverage human moment in the system.

## Context

Three kinds of human intervention, each with different UX needs:
- Build: design review (forward-looking, single wave, focused)
- Build: code review (backward-looking, single wave, focused)
- Garden: calibration (meta, all waves, panoramic)

Design and code review have existing UX patterns. Calibration is new — there is no established pattern for “review the trajectory of a coordinated system of agents.”

The interactive-checkpoint routing is now shipped end to end. Every `WaitInteractive` step produces an `interactive` attention item with typed context: `InteractiveAttentionContext` carries `step`, `terminalSessionId`, `designPath` (for `review-design`), and `mutationSummary` (for `review`). The executor owns creation and resolution. `AttentionQueueView` already shows these items with step-specific detail.

Calibration should ride the same `interactive` path, keyed by `context.step == "review"`. Its job is the dedicated all-waves review surface — not a new top-level attention kind, but a richer view when the queue item happens to be a garden calibration checkpoint.

## What to build

1. **Calibration prompt.** When a garden cycle reaches `review`, the calibration surfaces in the attention queue with a dedicated view.
2. **Assessment display.** Show progress, health, coherence, drift, and blind spots across the active waves.
3. **Proposal review.** Group proposed mutations by wave. Let the human approve, modify, or reject each one.
4. **Trajectory notes.** Let the human write back: “focus here next,” “this direction is wrong,” “this wave should be more ambitious.” These become durable memory.
5. **Calibration history.** Show past calibrations and what changed as a result.

## Done when

- Calibration appears as a structured view in the attention queue
- Assessment covers progress, health, coherence, drift, and blind spots
- Humans can approve or reject mutations and write trajectory notes
- Notes flow into durable memory
- Calibration history shows past reviews and their impact
