# LOO-724 assumptions — 2026-11-04

Jack Heart's acceptance, coordination boundary, unresolved permanent-transport
choice and evidence limits are kept once in [the plan](loo-724.md).

Reversible implementation choices:

- Treat retained `__planning-check` as Stop, with the same fence and subagent
  exclusion. Keep the retirement diagnostic for unknown hooks (older Sessions
  can retain title callbacks); genuine recognized-hook failures remain errors.
- Apply the existing three-second group-death observation window across processes.
  No local lifeline writer does not prove persistent helpers; surviving helpers
  still refuse replacement without signals.
- Retry the existing owner queue every 100 ms after nonblocking retention fails.
  This is scheduling, not new persistence or replay authority.

Preserve a live protected owner even when its socket cannot answer. `Uncertain`
contains no reconstructable editor text; history and screen snapshots cannot
recover queued keys or pending requests. Temporary loss of attachment must leave
those intact. Choosing irreversible loss for a permanently broken transport needs
Jack Heart's decision; it is not a headless implementation assumption.
