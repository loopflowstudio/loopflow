# LOO-600 questions and assumptions

- 2026-10-25: a live `claude -p --input-format stream-json` probe shows stdout carries
  `system/background_tasks_changed` (full task set), `task_started`, `task_updated`,
  `task_notification`, then Claude's own `init` + second `result`. So the code fix
  landed and the `headless.md` stopgap was not added (Task says "until that lands, or if
  no signal").
- Unhandled edge: a steer queued while background tasks run. Claude may answer the
  steer before the notification turn; results still pair FIFO with requests, so the
  seed's answer could be the steer's. Left as is; file if it shows up.
- Not checked: whether Codex/OpenCode have background work outliving a turn.
- `claude_body_activity_does_not_require_process_group_authority` failed once in the
  full `harness::` run at load average ~111 and passed alone; unrelated to this diff.

Check: `cargo test -p loopflow --lib harness::` 159 pass, 1 load flake (passes alone); clippy clean.
