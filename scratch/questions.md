# LOO-292 current dependencies and decisions

- 2026-09-24: latest published destination remains v0.12.19, before #1273.
  Its candidate preflight refusal is reproduced, not an environment hypothesis.
  A published release containing the startup repair and #1273 is required for
  the installed acceptance proof. Do not start/retry a release or promote an
  unreleased development binary into the production Home.
- Current installed active selection is development (`561eadcce`), unlike the
  published selection observed in the restored 2026-09-23 notes. Preserve that
  distinction. No active install state or Sessions were changed in this kickoff.
- No human design decision is required. The existing command split and working
  demo gate govern. Next executable work is the preflight routing repair and
  process-level regression in this same Task/worktree/serial branch.
- Prior scratch was cleared by `f73671bd0`, not silently absent evidence. Three
  prior Markdown artifacts are restored byte-for-byte from `7892ba68f`.
