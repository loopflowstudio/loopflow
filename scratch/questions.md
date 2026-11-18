# Open choices (LOO-938, kickoff 2, 2026-11-18, headless)

Made for review in `scratch/plan.md`; each is reversible before release.

- A stamp's `at` folds in the wall clock (`max(now ms, highest seen + 1)`).
  The design says a bare counter. Passed over because a Machine that writes
  more would win unseen races against later edits.
- Existing values are not stamped from the Git journal's clocks; an unstamped
  field reads as `(0, this Machine)`. Two Machines that disagree on an old
  value settle it by machine id, so etude's Machines sync with the released
  `lf` before installing.
- A repository with no remote writes no plan ref.
- Git fetch every 60 s per repository, by the Machine server, no setting. A
  command run without the server leaves Git unsent until the server or
  `lf task sync`.
- Tracker repositories are untouched in this Task: a person's own Machines do
  not exchange through Git or the hub there. The design says they do. Needs
  "which refs are mine" and one owner per field; left for the tracker Task
  (your slice, Linear manual order), which is not filed yet.
- Concept review, 2026-11-18, each reversible before release
  (`scratch/concept-review.html`, questions 1-5): a Machine's ref holds its
  whole plan, not only the fields it stamped newest; both carriers keep one
  cursor and say "unsent" (`hub_sends` goes); a clone takes its repository id
  from the plan ref if binding carries saved rows; a person's order of their
  claimed Tasks moves to the tracker Task.
- Whole-plan refs cost one small commit per Machine per change. Fine at ten
  Machines; untested beyond.
- First-wins claims add "the last entry I had seen" to the ownership marker.
  Entries a released `lf` wrote have none and read as having seen everything.

Still open from before:

- Should one Machine run the Linear/Asana drain for all of a person's
  Machines?
- A hub without the repository's checkout keeps sends open; Git has no such
  requirement. Should the hub line say so more plainly now that Git is the
  fallback?
- The mini's etude checkout and `lf` version are unverified (unreachable
  during both kickoffs).
- `lf session receive --planning-key` is gone, so a released `lf` moving a
  Task to a branch-built Machine fails on the unknown flag until both run the
  same release.
