# Open questions and assumptions · LOO-711

- **Reporting cadence assumption (implement, 2026-11-02):** report settled step
  transitions, failures and interactive stops; coalesce a step exit and next
  start after one second of quiet. Busy Sessions retain the latest reading.
  Jack Heart's authored demo reviews this reversible default; it is not his
  recorded cadence selection.
- **Native evidence boundary (implement, 2026-11-02):** production paths cover
  Codex, Claude, OpenCode and Pi through their existing native owners. Simulated
  provider/relay tests do not establish configured native rendering or draft
  preservation. In particular, verify Claude supplies its messaging inbox/token
  to the asynchronous hook in the configured build. Unacknowledged Claude/Pi
  writes remain uncertain and are never replayed. No provider restriction or
  terminal-input substitute is assumed.
- **Proposed memory update:** after demo, replace Product memory's “implementation
  unproved” description with the observed result and its actual limits. This
  implement step leaves `.lf/wave/` unchanged.
- Superseded wait-only evidence stays at
  `5f658180d9:scratch/loo-711-task-sessions-tell.md`; prototypes remain for review.
