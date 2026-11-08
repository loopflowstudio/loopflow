# Open questions

- `lf task run` on an unplaced Task now checks it out and so starts its
  conversation in the background, launching a provider beside a headless Graph.
  The `machine_commands` fixture treats any provider launch as a forbidden
  effect. Kept as designed (checkout is checkout, however reached) and made
  `lf task move` tolerate a conversation that failed or is still launching. If
  only `lf task checkout` should start the conversation, `finish_task_checkout`
  is the one place to change.

- The design asked how `lf task move` should treat a conversation that is still
  starting and got no answer. Built the option it marked Recommended: wait up to
  20 s for the provider PID, then stop it (`moves.rs` `await_provider_identity`).
  The alternative, ending it at once without waiting, is a 15-line swap.
