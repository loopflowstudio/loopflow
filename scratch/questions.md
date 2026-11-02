# Open questions and assumptions · LOO-711

- **Native delivery remains undefined** (implementation, 2026-11-02).
  Jack Heart selected Session posts plus the run page. The supported provider
  scope and receipt/reconnect behavior still need a design; the existing Codex
  relay and Claude probes are starting points. See
  `loo-711-task-sessions-tell.md` for the contract and remaining acceptance.
- **Reporting cadence remains open.** Every step boundary was kickoff's
  assumption, not Jack's decision. Decide which moves deserve a turn while
  preserving reports of failures and interactive stops; coalesce exit/next-start.
- **Context curation assumption** (compress, 2026-11-02): keep the current
  implementation reconciliation authoritative and archive the superseded
  wait-only recipe at `5f658180d9:scratch/loo-711-task-sessions-tell.md`.
  Preserve prototypes for review. No provider restriction or delivery substitute
  is inferred from the incomplete implementation.
