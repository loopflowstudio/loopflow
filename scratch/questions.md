# Open assumptions

- `lf task <linear-item-id>` is the public command; the three-phase flow is
  named `task-pass` to avoid colliding with the subcommand. A wave must be
  registered before `lf task` can place its worktree.
- KR set = items labeled `kr` in the wave's Linear project; an empty set
  refuses to start rather than reading as done. Richer KR representation is
  deferred to project-tier wiring.
- Executive calls awaiting review, not blocking: `HEARTBEAT_IDLE` = 4h for
  pass-based waves; the pass's reply text is its stdout tail (stderr appended
  when present).
- The wire rename shipped with NO journal compat: pre-rename
  `journal.jsonl` files (mind_state tags) no longer fold. Existing wave
  journals on this machine reset on first boot after deploy — accepted,
  single-user.
- The `mind:` GOAL.md vendor knob was deleted, not renamed: pass runs select
  agents through the standard machinery (`agent:`, `step_agents:`, step
  `default_agent:`).
