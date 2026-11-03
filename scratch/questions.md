# Open questions and assumptions · LOO-711

- **Reporting cadence assumption (implement, 2026-11-02):** report settled step
  transitions, failures and interactive stops; coalesce a step exit and next
  start after one second of quiet. Codex/OpenCode retain the latest reading while busy; Claude/Pi use native queues after the November 3 stale-availability repair. Burst behavior remains to measure.
  Jack Heart's authored demo reviews this reversible default; it is not his
  recorded cadence selection.
- **Native evidence boundary (implement, 2026-11-02):** production paths cover
  Codex, Claude, OpenCode and Pi through their existing native owners. Simulated
  provider/relay tests do not establish configured native rendering or draft
  preservation. The November 3 configured native demo verified Claude supplies its inbox capability; all four idle wake/rendering/draft cases are now observed. Unacknowledged Claude/Pi
  writes remain uncertain and are never replayed. No provider restriction or
  terminal-input substitute is assumed.
- **Demo boundary:** All four native idle wake and draft checks were observed on
  November 3. Jack Heart subsequently gave positive feedback and authorized
  shipping with disclosed limits. Headless decision pass 1 advances on that
  evidence; busy bursts, comprehensive reconnect and mounted run-page navigation
  remain unproved. Product memory's earlier unverified-provider account is stale;
  the current evidence and decision are in `loo-711-native-update-demo.md`.
- Superseded wait-only evidence stays at
  `5f658180d9:scratch/loo-711-task-sessions-tell.md`; prototypes remain for review.
