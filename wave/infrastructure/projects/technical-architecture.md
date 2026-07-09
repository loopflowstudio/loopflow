# Technical Architecture

Loopflow's architecture is legible from the top down: the key data structures
and APIs explain the system, the implementation follows that map, and obsolete
pre-flowloop concepts do not linger as alternate design.

## KRs

- Top-down architecture documentation is complete, published, and centered on
  the key data structures and public APIs.
- Every data structure and API in the architecture is ratified as minimally
  simple for its purpose, with no duplicate owner, hidden mirror, or
  compatibility shim unless explicitly justified.
- The codebase, prompts, docs, and UI contain no stale pre-flowloop technical
  design language that can mislead future work.
