# Assumptions

- W2-135 process leases are already on `main`. The handoff records the referenced
  body generation but deliberately adds no second lease or process owner.
- `lf handoff` is the durable primitive name. Presentations may wrap it, but
  Wave, Project, and Task remain the only parent lifecycle nouns.
- W2-175 owns the primitive, parent wait/resume integration, reconciliation, and
  ten-handoff proof. Mac and external presentation adapters remain separate
  Tasks and consume the shared descriptor.
- A Wave parent uses the Wave pass/resident replay boundary; it does not gain a
  Project/Task-style W2-135 body lease merely to support handoff.
- The next migration ordinal on this branch is `0.11.008`; concurrent branches
  may require an ordinal-only rebase before landing.
