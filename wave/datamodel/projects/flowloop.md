# Flowloop

Everything agentic is a looping flow with an agent-set termination bit
(PR #845). This bet finishes the runtime and proves the thesis on it:
**writing a goal is a way to compute** — demonstrated by waves that run,
not claimed by design docs.

## KRs

- >= 5 waves run consistently for 1 week straight across Cadenza and
  loopflow; at least 2 waves per codebase run from GOAL.md with zero
  repo-authored skills added for the work.
- 5/5 new wave starts produce a GOAL.md + projects/ that pass the honest
  question before the first dispatch, and stay current over time.
- Steward ships: one warm executive mind per wave owns the human thread;
  `lf chat` becomes the agent bus, fully auditable in its own surface
  (Linear 606d59b7).
- `lf task` drives a red PR green unattended: CI classification,
  reproduce-then-fail, rebase/review folding (v1b, 33a98266); then holds
  unattended — non-convergence fingerprint, restart ladder, poison
  dead-letter (v1c, ac851c0b).
- The project tier wires: waves spawn project loops, subtree caps hold
  (bf2d8709).
- The bare `lf <name>` fallback is decided (a9dc614d).
