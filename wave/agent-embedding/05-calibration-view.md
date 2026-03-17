# 05: Calibration View

**Finish line:** The tend flow's human checkpoint has a dedicated UX. Not a notification, not a PR review — a trajectory review across all waves. The highest-leverage human moment in the system.

## Context

Three kinds of human intervention, each with different UX needs:
- Build: design review (forward-looking, single wave, focused)
- Build: code review (backward-looking, single wave, focused)
- Tend: calibration (meta, all waves, panoramic)

Design and code review have existing UX patterns (PR review, design doc review). Calibration is new — there's no established pattern for "review the trajectory of a coordinated system of agents."

## What to build

1. **Calibration prompt.** When `tend/draft-chord` completes, the calibration surfaces in the attention queue with a dedicated view. Not a generic failure row — a structured trajectory review built on `AttentionItem.calibration`.

2. **Assessment display.** The chord's observations, per wave and overall:
   - Progress: what shipped since last calibration, velocity trend
   - Health: which waves are thriving, stalling, producing shallow work
   - Coherence: are the waves collectively building what we intend?
   - Drift: human-system gap — are approvals getting mechanical?
   - Blind spots: are agents testing what users experience? Are there capability gaps?

3. **Proposal review.** The chord's proposed mutations, grouped by wave. Each with rationale. The human approves, modifies, or rejects per-mutation. Approved mutations execute immediately.

4. **Trajectory notes.** The human can write back — "focus here next", "this direction is wrong", "this wave should be more ambitious". These become core memories in Letta, shaping future tend cycles.

5. **Calibration history.** Past calibrations and what changed as a result. The chord and human can see whether course corrections had the intended effect.

## Done when

- Calibration appears as a structured view in the attention queue
- Assessment covers progress, health, coherence, drift, and blind spots
- Human can approve/reject mutations and write trajectory notes
- Notes flow into Letta as core memories
- Calibration history shows past reviews and their impact
