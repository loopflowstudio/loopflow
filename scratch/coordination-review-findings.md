# Remaining performance review

Source review, 2026-11-09; reproduce before repairing.

- `TaskUpdates::read` retains settling candidates for Task/Graph IDs that disappear from a Session's observed membership. If membership changes or a Graph is removed before its candidate settles, that ID is never visited again. `pending` then bypasses the cheap revision-only path indefinitely because `candidates` never empties. Add a regression that begins settling, removes the observed membership, and proves subsequent settled polls recover the one-query budget while a remaining Task still delivers changes. Clear candidates on ineligible/missing Sessions and retain only IDs visited by a successful observation; preserve candidates on fallible reads until retry. Do this after the frozen subscription experiment, separately from its A/B claim.
- The subscription fixture caps reader worker pools at two and its server blocking pool at four. Preserve its frozen comparison, and distinguish it from normal runtime defaults. A separate bounded default-configuration pilot was requested of the implementing run. Resource-inventory `ps` children must not be presented as application Git work.
- The two executor saturation experiments in `server/subscription_measurement.rs` construct hypothetical executors directly. They establish the design constraint, but a future change to production `Reader` would not make them fail. Before shipping, prefer a focused saturation regression through production subscription creation/pull with real Store work; retain the hypothetical comparison only as an opt-in experiment if useful. The queued-cancellation failure must become a passing, enabled production regression after repair.

2026-11-09 implementation follow-up: the orphaned-candidate repair is isolated
in source commit `c7c32ad1137f5ff3d40ca5ad8ce266376b122e66`. Membership and Graph
removal recover the one-query poll budget; retained Task comments still arrive,
partial failures retain candidates, and ineligible/missing Sessions release them.
Shared subscriptions and production SQLite saturation/cancellation regressions
are in `397bceec1d864aa049b292a3b9b87195cf59a99b`. Queued cancellation is enabled
and passes. The runtime-only probes were deleted from current source; frozen
copies remain. Paired optimized measurement and the broader owner-continuity
gate remain; exact handoff is in `coordination-subscription-design.md`.
