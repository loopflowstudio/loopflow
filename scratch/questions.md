# LOO-292 current dependencies and decisions

- 2026-10-02: the latest supplied Task contract supersedes the coupled refresh
  design below. Install updates published artifacts only; rebase updates
  checkouts; scheduling defaults to login plus weekly. Published and installed
  CLI are now 0.12.29. Preflight succeeds. The earlier missing-daemon diagnosis
  is superseded: v0.12.29 retires the separate daemon. Jack reports successful
  unbound `lf install` (“already installed”). Read-only app signatures and
  CLI/app/helper hashes match the published receipt; retained history is readable.
  Task-bound install attempts were refused, but no installation repair is
  indicated. The unbound coordinator can run `lf install schedule` (weekly),
  repeat it, and inspect launchd/logs; natural login/wake proof remains outstanding.
  No new product decision is required. Preserve the saved Flow's separate
  migration blocker without restart. No activation or release was attempted here.
  Current evidence and remaining proof: [October 2 demo](demo-published-install-20261002.md).

## Historical September 24 observations

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
