# Open Questions

## Roadmap unreachable — Asana token expired (update-wave, 2026-07-05)

`lf op pm show/update` fails with "Stored asana token has expired. Run
`lf op auth asana` again." That re-auth is an interactive browser flow, so it
can't run headless. The rebase-efficiency follow-ups that should be filed as
Asana tasks are parked in `wave/systems/MEMORY.md` under "Rebase-efficiency
follow-ups (file to Asana once auth restored)" and "Next" — file them once a
human restores auth. No roadmap reconciliation was possible this run.


## `--main` vs `--fork` are behaviorally identical (compress pass, 2026-07-05)

`PlacementRequest::Main` and `PlacementRequest::Fork` resolve to the same arm in
`plan_placement` (root branch off `default_branch`), and `wt_create` groups them
in the `sync_default_base` match too. `--fork` is documented as "from the review
base" but that distinction is unimplemented, so today `--fork` is a pure alias of
`--main`.

Reduction would be to drop `--fork` / `PlacementRequest::Fork` and keep `--main`.
I left it: it's user-facing CLI surface with distinct documented intent, and
removing it deletes planned capability rather than an accident. If the review-base
fork is not going to be wired, delete the flag + variant; if it is, wire the
different base.
