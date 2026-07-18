# PRD-38 questions

No landing decision is currently blocked on product semantics.

## Resolved during implementation

- The reproducible size gate is physical lines under `rust/loopflow/src`, with
  baseline 144,210 and a required reduction of at least 10,000. Current is
  134,190.
- `LF_RUN_CONTEXT` is the positive in-Run sentinel. Missing or invalid
  `LF_RUN_LEASE` cannot fall through to User authority.
- Migration 36 is executable and one-way on this branch. It drops Session
  tables; it is not an unapplied draft.
- Historical migrations keep Session vocabulary because an old database must
  apply them before the deletion migration.
- One executor means one Run authority and one `run_work(WorkRef)` entrypoint.
  Project and Task keep distinct domain policy loops.
- The product wire sends one recommended Task action plus a reason. It does not
  enumerate every unavailable alternative.

## Follow-up, not a PRD-38 blocker

- Decide whether the app's `ActiveSessions` feature should become `ActiveWork`.
  Terminal and tmux/provider sessions are real UI substrate terms, so that is a
  product-language pass rather than part of the storage cut.
