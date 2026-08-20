# Open blockers

- LOO-222 still needs two human-present proofs: the Claude focus/clipboard
  observation and the delayed Codex login for
  `manabot-eng@loopflow.studio` described in the review matrix.
- A headless `scripts/dev-lf auth connect codex manabot-eng@loopflow.studio`
  attempt on 2026-08-19 failed before provider startup because the sandbox
  could not resolve the provider account store (`Operation not permitted`).
  The real-path proof requires host filesystem and browser authority.
- Repeated `lf ask --user` attempts on 2026-08-19 were rejected because the
  AgentInvocations had no active Turn authority. A parent or User Run with valid
  control authority must open or perform the intervention.
- The auth implementation is checkpointed and pushed. This review-note cleanup
  still needs the owning workflow to checkpoint it because the sandbox cannot
  write the shared worktree index or runtime ledger.
