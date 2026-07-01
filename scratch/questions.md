# Open questions — goals wave

## Wave budget: built-in vs user-authored

As the release wave matures deployment + budget tracking, do we build budgeting
into loopflow as a first-class part of the loop, or keep the core thin and
encourage users to write their own budget goals/steps?

Lean: ship a minimal hard `spend_cap` + block→human as a safety floor in the
core, expose the cost signal + pause primitive so richer budgeting is writable
on top. Not decided — resolve at build time. See `wave/goals/2-wave-budget.md`.

## Other build-time forks (recorded in the wave items, not blockers)

- Backend (a) launch: A1 (lfd drives vendor cloud) vs A2 (scaffold + hand off).
- Asana grain: Goal ↔ project or portfolio? item ↔ task or subtask? write-back depth.
- Single *leaf* Looping Agent spanning repos vs chord-only cross-repo.
- Backend (b) persistence: long-lived agent vs threaded ticker.

## M1 parser cut vs full wave field deletion

Implemented the safe parser boundary first: `wave/<name>/goal.md` no longer seeds
legacy flow aliases, crons/triggers, serialized mode, wave-level area/direction,
or step-agent maps. The wider deletion of `Wave.area`, `Wave.direction`, request
payload fields, DTO fields, store columns, and Concerto affordances remains a
separate mechanical cut because those fields are still accepted through the live
API and UI.
