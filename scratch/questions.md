# Open evidence

- Observed 2026-08-24: no SQLite database below `/Users/jack/.lf` or
  `/Users/jack/.lf-dev` currently contains a `replay_contracts` table. The
  retained LOO-271 Home contains
  `invocation_2f2ac303607c4673a19b83f462497c5b`, but its capture is
  complete/failed and its artifact directory contains only conversation,
  provider, system-prompt, and task-prompt files. Running this branch's built
  `lf replay check` against that Home fails with `no such table:
  replay_contracts`.
- Executive assumption: the LOO-271 live proof was ephemeral or was not
  retained with its development Home. Implementation will create a fresh real
  eligible source through the merged replay-safe producer and replay that exact
  invocation. The focused synthetic fixture is necessary behavioral coverage
  but does not count as the task's real execution proof.
