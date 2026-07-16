## Try it

A stale `LF_WAVE_ID` UUID (one this machine's registry has no row for) is now
visible in the run record instead of vanishing:

```bash
# registered wave resolves normally
LF_WAVE_ID=<valid-uuid> lf runs            # run rows carry wave:<name>

# stale UUID: the run is attributed to NO wave, and the stale failure is recorded
LF_WAVE_ID=<unregistered-uuid> lf runs     # wave:- , and the started row's
                                           # error names the stale id + --wave fix
```

The warn is also emitted once at run start:

```
WARN ambient wave identity failed validation; run attributed to no wave — pass --wave <name> to recover
```

## Intent

W2-239. After the shared ambient-wave resolver (W2-151 / PRs #915, #979) landed,
trace and run attribution still swallowed the resolver's classified
`StaleIdentity` / `Registry` errors to `None` via `resolve_run_wave_name() =
…ok()`. A run whose `LF_WAVE_ID=<uuid>` named a Wave this machine had no row for
became indistinguishable from a bare command with no managed identity — the
durable identity was supplied and failed validation, and the failure was erased.
This propagates that classified failure so a stale identity is visible and
actionable, and a stale run is never quietly re-attributed to a wave inferred
from the worktree.

## Assumptions

- Attribution stays **non-fatal**: a stale identity never aborts the command. The
  run completes; the failure is recorded + warned, not raised.
- `wave` is `None` for both absent context and stale identity. The difference is
  that stale carries a recorded failure in the existing `run_events.error` column.
  This is the honest wire shape — no schema/DTO change, no backfill.
- Run status is derived from the **terminal** event, so an `error` on the
  `Started` row never marks a run as errored.

## Key decisions

- **`RunAttribution` is a first-class value** (`wave_context.rs`) so both
  attribution sites (`journal::ensure_run_context` and the `lf` run wrapper) share
  one classification and it is unit-testable. `NoContext → (None, None)` (worktree
  inference stays a legitimate fallback for it alone); `StaleIdentity`/`Registry`
  → `(None, Some(failure))`; valid UUID/name → `(Some(name), None)`.
- **No worktree inference is introduced.** The resolver invariant — "repository
  location cannot identify a Wave" — holds; task-worktree basenames
  (`loopflow.<wave>.<task>.<ts>`) are not wave names. The fix ensures the stale
  case is not silently re-attributed; it does not add path-based identity.
- **`resolve_run_wave_name()` is kept** as a thin wrapper over
  `run_attribution().wave`, so non-attribution callers (`lf home`) keep their
  current behavior. Surfacing stale identity in `lf home` is a sibling concern,
  out of this task's scope.
- The `StaleIdentity` `Display` already names the id and `--wave <name>`; for
  `Registry` the recovery hint is appended.

## Not included

- No `run_events` schema change and no DTO mirror change (Rust/Swift/Python). The
  stale signal rides the existing `error` column.
- No read-side display change in `lf runs` (e.g. a distinct "stale" badge). The
  stale run shows `wave:-` with the failure on its started row; a dedicated
  read-surface is a follow-up.
- `lf home` routing/probe unchanged.

## Tests

- `run_attribution_classifies_absent_context_and_hand_set_names` — absent
  `(None, None)`; hand-set name (even unregistered — a "stale name" is not a
  state the resolver produces) → `(Some(name), None)`.
- `stale_ambient_uuid_is_propagated_not_inferred_from_the_worktree` — stale UUID
  → ledger `wave: None` + `error` naming the stale id and `--wave`; never the
  worktree or the registered wave.
- Existing `explicit_wave_env_overrides_the_worktree_for_ledger_attribution`
  (valid UUID), `wave_resolution_tests` matrix (explicit override, stale error,
  no-context error), and the full journal/wave_context suites still pass.
