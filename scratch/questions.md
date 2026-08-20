# Open blockers

- LOO-222 still needs two human-present proofs: the Claude focus/clipboard
  observation and the delayed Codex login for
  `manabot-eng@loopflow.studio` described in the review matrix.
- Repeated `lf ask --user` attempts on 2026-08-19 were rejected because the
  AgentInvocations had no active Turn authority. A parent or User Run with valid
  control authority must open or perform the intervention.
- The implementation is verified but not checkpointed. `lf commit` reached
  the correct Loopflow path, then the sandbox denied writes to the shared Git
  worktree index lock; the same call also reported the runtime ledger database
  as read-only. The owning runner/final flow must commit once it has repository
  metadata write authority. No raw Git workaround was attempted.
