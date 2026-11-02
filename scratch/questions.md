# Open questions (LOO-708)

Concept review, 2026-11-02: no unresolved product choice. Jack Heart's requirement
to keep resume and exit recording intact resolves the earlier assumptions toward
preservation. No approval of a compatibility break is inferred. The working design
records the next implement direction and evidence.

Code-inspection follow-up after compression: fork and new-thread launch paths
still exec through `serve_locked` before JSON-aware delegation. This is a remaining
preservation defect, not a product choice. The plan records the exact callers and
next focused proofs; ordinary resume/reconnection checks do not cover them.

- The repair carries post-return Session JSON across the relay exec;
  `--prepare --json` remains unchanged. A command continuation may open the
  store after the relay ends, while the live relay remains store-free. Existing
  CLI validation rejects root `lf --json resume`; the command-local
  `lf resume ID --json` form is covered without broadening flag semantics.
- The repair shares launcher result and capture interpretation with owner
  settlement. Native status, command result and attachment loss remain distinct.
- A provider that exits before publishing history used to fail `lf session
  connect` with "exited … before becoming resumable" from the connect's poll
  loop. The connect now execs into the launcher, so the launcher's own error
  (for OpenCode, "private server failed to start") is what the caller sees.
  Keep the direct error: actionable startup failure and recovery remain the
  contract; exact historical wording is not required. The lifecycle test asserts
  the new wording.
- The owner runs `PlanningSync` for the launch's repository once it holds the
  exit record, as the launcher did while it waited. Its `git` children are
  transient and never `lf`.
