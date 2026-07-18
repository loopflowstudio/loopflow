# Open Work — 2026-07-17

## Review recovery pass

| Task | Current evidence | Action |
| --- | --- | --- |
| INT-5 | Recovered successor is live under Claude with all interaction policies deferred | Let the body continue |
| INT-6 | Recovered successor is live under Codex with all interaction policies deferred | Let the body continue |
| INT-4 | Fresh Claude body is live on the fourth PR record with all interaction policies deferred | Let the body continue |
| PRD-20 | Design review returned changes: preserve one shipping judgment and SHA-pinned settlement, remove active-Session authority; Claude recovery body is live | Redesign against Run/Review, rebase, and repair PR #1052 CI |
| ENG-115 | Design approved; exact-frontier repair body relaunched under Claude with all interaction policies deferred | Finish implementation and carry Run naming through the launch boundary |
| PRD-38 | Session-deletion body remains live; its legacy completed Kickoff review is being consumed under a temporary Require policy | Finish implementation, then restore Kickoff to Defer before Gate |
| INT-9 | Live successor body | Leave running |
| INT-10 | Live successor body | Leave running |
| RUL-5 | Live successor with Project-routed review | Leave to parent lane unless it stalls |
| GAM-4 | Completed | No action |
| RUL-1 | PR #119 merged; obsolete User-routed Session abandoned; worktree removed | No restart |
| INT-1 | PR #113 merged; obsolete User-routed Session abandoned; worktree removed | No restart |

Settled in this pass: PRD-6, PRD-9, and PRD-12 are terminal after their merged
work was verified. PRD-6 required reconstructing a clean detached worktree from
its recorded merge commit because completion currently refuses when a merged
worktree has already been removed.

## Safety boundary

Do not batch-rewrite review ownership at an occupied waitpoint. The legacy
store keys the waitpoint to policy and reviewer, so a Require-to-Defer rewrite
before the completed exercise advances causes `interaction review waitpoint
already belongs to a different exercise`. Preserve the review, let its current
runner consume completion, then persist the headless policy from an inactive
boundary.

## Architecture bugs surfaced

- `task complete` can bypass an outstanding Project review even when status
  reports completion unavailable. PRD-12 demonstrated the split between the
  derived legal-action model and the mutation guard.
- `task complete` requires the historical worktree to exist even after every PR
  merged and the checkout was safely removed. Completion should fence on the
  recorded merged evidence, not filesystem resurrection.
- Converting a completed User waitpoint to Defer before its runner advances
  collides with the old review identity. Headless conversion needs an atomic
  carry rule rather than a temporary direct policy rewrite.
- `task interrupt` can persist a replacement while the resident body remains
  active, but `task run --headless` requires no active body. There is no public
  non-terminal stop/park operation for this conversion boundary.
