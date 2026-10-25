# LOO-600 questions and assumptions

Status: acceptance 1 and 3 met; acceptance 2's stopgap not needed (signal exists).

- 2026-10-25: a live `claude -p --input-format stream-json` probe shows stdout carries
  `system/background_tasks_changed` (full task set), `task_started`, `task_updated`,
  `task_notification`, then Claude's own `init` + second `result`. So the code fix
  landed and the `headless.md` stopgap was not added (Task says "until that lands, or if
  no signal").
- Steer accepted while background tasks run (realign, 2026-10-25): the first version
  skipped every result under background, so the steer's result was never counted and
  the runner turn hung until the process died. Now a result under background
  decrements the owed count down to 1 and the history completes the front request
  when a later one is queued; the last request takes the notification turn's answer.
  Assumes one `result` per accepted user message plus Claude's own notification
  turn(s), the same model existing steer handling uses; not probed live with a steer.
  Pairing stays FIFO, so the seed may record the steer's answer as its output.
- Not checked: whether Codex/OpenCode have background work outliving a turn.
- Not checked: whether LOO-586's lost `pr_landing.rs` resolution was this failure.
- `claude_body_activity_does_not_require_process_group_authority` failed once in the
  full `harness::` run at load average ~111 and passed alone; unrelated to this diff.

Check: `cargo test -p loopflow --lib harness::claude` 41 pass (new steer test fails/hangs without the fix); clippy clean.
