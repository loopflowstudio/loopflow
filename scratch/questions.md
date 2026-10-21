# LOO-557 open questions and assumptions

- **No age grace for dead, unrecorded receipts (assumption, 2026-10-21).** The Task
  asks for pruning "after a grace period". The plan relies on affirmative `Dead`
  OS evidence plus a definite no-row read. A live or just-started Process is
  `Live` and keeps its receipt. A time grace would protect nothing extra. To
  restore it, filter on receipt mtime older than `disk::GRACE`.
- **Receipts of unfinished rows stay (assumption).** They are the row's only exit
  proof. Settling those rows as interrupted would let one Process write
  another's history, so this plan does not do it.
- **Archive directory.** `~/.lf/runtime/exec-processes-archive-20261021` is left
  alone. Jack can delete it once the prune rule ships. It sits under `runtime/`,
  so budget rotation may or may not rotate it.
- **Observation root fix is in scope (decision, 2026-10-21).** `TaskFileObservation`
  matched FSEvents paths against a `/private`-stripped root, so checkouts under
  `/var` or `/tmp` got no FSEvents paths. It is fixed here because the acceptance
  test needs it. Real `~/src` checkouts were unaffected and still had the loop.
- **Remote ignored open documents (accepted consequence, 2026-10-21).** The remote
  watcher drops git-ignored paths, so an open remote `.env` or other ignored file
  stops refreshing on external edits. Local still refreshes it. To restore it,
  the frame would carry ignored paths separately (`ignored_paths`). Swift would
  re-read matching open documents and leave the comparison alone. The cost is
  forwarding build churn over the wire. Not a blocker. A follow-up Task can be
  opened if Jack wants parity.
