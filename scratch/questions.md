# Open assumptions

- `lf task <linear-item-id>` is the public command; the three-phase flow is
  named `task-pass` to avoid colliding with the subcommand. A wave must be
  registered before `lf task` can place its worktree.
- KR set = items labeled `kr` in the wave's Linear project; an empty set
  refuses to start rather than reading as done. Richer KR representation is
  deferred to project-tier wiring.
- Wave conversion (`scratch/wave-conversion.md`) executive calls awaiting
  review, not blocking: `HEARTBEAT_IDLE` → 4h for pass-based waves; the
  thread reply is the `wave_mutate` phase's final text (builder verifies what
  `lf -b` emits); `MindState` → `FlowloopState` DTO rename may split into an
  immediate follow-up PR — the only permitted deferral.
- Implementation note: this slice switches the live resident path to
  pass-based `wave-pass` execution and renames the public wave flags to
  `--no-flowloop` / `--flowloop-only`. The broader `MindState`/health-field
  rename and removal of the legacy harness scheduler remain as the DTO
  follow-up because the wire and Swift mirrors still expose `mind`.
