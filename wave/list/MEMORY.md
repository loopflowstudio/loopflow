# Decisions

- On 2026-09-02, Project and Task controller liveness and signaling were bound
  to one Work-linked, birth-validated OS Exec owner. Provider Invocation,
  provider session, and Run state remain advisory: they cannot authorize a
  signal or veto recovery after positive OS absence. A live birth with missing
  or contradictory ownership fails closed, as do multiple validated owners;
  tmux remains transport rather than authority.
- On 2026-09-02, release promotion was bound to stop-and-restart handoff rather
  than adjacent controller generations. The machine switch receipt captures
  each live Work's exact prior attempt, proves that owner absent before store
  advance, and settles only after a distinct target attempt is live or the
  Work is durably parked. Ordinary launches share the promotion lock and refuse
  unsettled switch receipts; promotion and recovery hold the lock exclusively.

# Learnings

- On 2026-09-02, Project and Task controller launch stopped treating detached
  tmux creation as startup proof. An attempt-scoped receipt now distinguishes a
  real body with a provider Run and exact live Exec owner from a parked human
  node or a failed child; the launcher records a resumable Work failure when
  the child cannot do so itself.
- Installed controller executables are content-addressed as
  `lf-<64-hex-digest>`. Process inspection must recognize that name alongside
  the development `lf` binary or it will hide valid ownership evidence.
- A controller handoff needs one nullable collection, not a capture flag beside
  a collection: `None` means capture has not occurred, while `Some([])` is
  durable proof that capture completed with no live controllers.
- A fresh release target must clone the prior selected store, not a fixed
  production store; otherwise a live controller can restart against stale or
  missing Work after a development-to-development promotion.
