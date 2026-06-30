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
