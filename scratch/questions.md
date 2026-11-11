# Open questions

- Today an idle conversation is woken by a Task update: Codex gets a `turn/start` with
  `loopflow_task_update` from `__codex-observe`, Claude gets an async `task-updates` Stop hook
  that waits. With no per-Session process, should an idle managed conversation still be woken
  by a Task update, or does it receive the update only at its next turn? The docs in
  `docs/sessions.md` follow the Task's demo ("a steer and a Task comment still reach each
  Session at its next turn") and say next turn, which matches steers (`docs/work.md`:
  "Idle Sessions stay idle; nothing is replayed into them"). Waking Claude without a process
  would need the holder to type into the PTY.
