# Technical Architecture

Loopflow's architecture is legible from the top down: the key data structures
and APIs explain the system, the implementation follows that map, and obsolete
pre-loop concepts do not linger as alternate design.

## KRs

- Top-down architecture documentation exists, is prompt-read by the waves
  that work on the system, and stays true: four consecutive weekly drift
  checks find zero owners, mirrors, or shims the map doesn't name.
- A month of landed PRs maps cleanly onto the documented structures — any
  PR that needs a concept the map lacks either updates the map or is
  evidence this KR failed.
- Stale pre-loop design language reaches zero across code, prompts,
  docs, and UI — and stays at zero for a month after the sweep, verified by
  the same check that got it there.
