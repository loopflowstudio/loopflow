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
  (your slice, Linear manual order), LOO-980.
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

- Slice 1 (implement, 2026-11-18), each reversible before release:
  - Tracker observations are stamped too, as the plan says. On a Linear
    repository nothing reads those rows (about 15 per Task). Keep, or stamp
    only repositories without a tracker?
  - A Wave's cleared `current_project` or `retired_at` still does not travel:
    the record omits those fields when empty, as before stamps. Stamps would
    now make sending the empty value safe.
  - `lf repo tracker --json` names at most twenty unsent records
    (`records`) beside the count (`unsent`); the docs said only `unsent`.
  - `lf task sync` without a tracker prints `<id>: nothing unsent` or
    `<id>: hub, N unsent` for the whole repository, not that Task's fields.
  - The hub acceptance test took 136 s on a loaded Machine (nextest kills at
    300 s). Not measured before this slice, so unknown whether it grew.

- Slice 2 (implement, 2026-11-18), each reversible before release:
  - The plan's remote is `origin`, else the first remote by name. No setting.
  - The server's first fetch is a minute after it starts, not at start, so
    test fixtures with a served home do not push refs into their remotes.
  - `lf task sync` fails when either carrier fails, and Git still runs when
    the hub is down. It prints `<id>: Git origin, N unsent; hub, N unsent`.
  - A record another Machine's ref carries that cannot be written here (a
    comment whose Task is missing) is skipped and logged; the rest applies.
  - A plan ref commit's author is the machine id at `loopflow@localhost`.
  - A deleted record's file is never removed from a ref: deletion is a field.
  - `lf repo tracker` runs `git remote` on each read to name the Git line.

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
