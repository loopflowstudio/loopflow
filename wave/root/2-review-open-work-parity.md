# review-open-work ↔ build/garden parity

**Finish line:** `review-open-work` is the manual sibling of the automated status-meeting flows. The manual version and the automated `govern-*` / `garden-act` versions share as much logic as makes sense, and the distinction between them is clear.

## Context

`review-open-work` is a step the conductor runs manually to get to inbox-zero on branches, PRs, worktrees, and waves. Two phases: clear-the-decks (Pass 1) and wave audit (Pass 2).

The chord-level automated flows — `govern-coordination`, `govern-control`, `govern-intelligence`, `govern-identity`, `garden-act` — run scheduled scans and assessments, and propose mutations through xor routing.

These families are doing related work at different altitudes and cadences. The relationship is currently implicit.

## Questions

- What does `review-open-work` do that `govern-*` could do instead? (worktree pruning, stale-branch cleanup — mechanical, automatable)
- What does `govern-*` already do that `review-open-work` should mirror? (wave-health assessment, cross-wave conflict detection)
- Should the manual pass be Pass 0 of a broader review flow that then triggers automated sweeps?
- Are the status-meeting outputs the same artifact at different altitudes, or different artifacts?
- When the conductor runs `review-open-work`, should it trigger a fresh `garden/scan` first so Pass 2 reads real signals?

## Done when

- The relationship between manual `review-open-work` and automated `govern-*` / `garden-act` is articulated.
- Shared sub-steps are extracted (or deliberately duplicated with a documented reason).
- The conductor has one story for "how do I know what's going on" across manual and automated modes.
