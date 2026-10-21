# LOO-552 open questions (assumed 2026-10-21, kickoff; Jack to confirm)

- Default ref name: assumed `refs/loopflow/planning/shared/main` (existing
  shared namespace). Reversible until a second person publishes to it.
- Personal opt-in: assumed `personal: true` under `planning:`; error with `shared`.
  Planning key kept, used only by personal refs.
- Migration: assumed automatic on first publish after upgrade — switching
  destinations already exports every owned mutation. No command; old personal
  ref left on the remote.
- LOO-550 docs overlap: whichever lands second rebases its planning docs.
- Automatic migration vs LOO-412's "joining cannot silently publish existing
  plans" (realign 2026-10-21): unresolved; code implements automatic move. See the
  plan's "Open conflict with LOO-412" for the three choices.
