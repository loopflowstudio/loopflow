# LOO-630 findings (2026-10-27 UTC)

## Live state
- Shared mode (not isolated). Every running Claude reads the one native login
  (`running_agents_follow_native_login`), so all live Claudes share one account.
- Native Keychain `Claude Code-credentials` now holds `primary`'s exact tokens;
  all launches since 04:08Z pinned `primary`. Live Claudes are on jackstah@gmail.com.

## Why they were on loopflow
- Declared routes (default and loopflowstudio/loopflow) are `loopflow → primary`.
  `lf account route` showed primary first only because loopflow was demoted.
- Shared selection (`select_shared_account`) keeps the active account until it is
  strained (≥95% on any window), otherwise picks by strain, then route order.
- 03:41:23Z: a launch's `activate` couldn't identify the native login (access token
  expired 2026-10-24 04:27Z, refresh unusable), kept it aside as
  `native-1bbb162ce109.credentials.json`, and installed the route's first unstrained
  account: loopflow (<95% then). Every running Claude followed. Pins 03:38–03:47Z = loopflow.
- Jack's `/login` (~04:4xZ) put primary back natively.
- Verdict: not "selection ignored the route" — it followed a route that prefers loopflow.

## Defects found
1. `activate` writes the native login, then records the switch; the insert failed
   (no rows since 2026-10-22 despite pins flipping, DB `database is locked` storm) —
   native changes, switch log doesn't, `used_account` attribution goes wrong, launch errors.
2. Pins fail under the same lock contention (`failed to pin Claude provider session account`).
3. `inspect_provider_route` drops an account whose profile digest went stale
   (primary vanished from `lf account route` at 04:47:34Z) — display only; launch re-verifies.
4. ~190 orphaned scoped `Claude Code-credentials-<hash>` Keychain items.

## Open decision (Jack)
Route order loopflow → primary, or primary → loopflow / capacity-first.

Decision (Jack, 2026-10-27): most capacity first. Shared selection ranks by strain,
plan, then weekly headroom; route order breaks ties and the active account wins
only ties. Also fix defect 1 (switch log vs native install).
