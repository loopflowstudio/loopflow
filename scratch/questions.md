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

- Slice 3 (implement, 2026-11-18), each reversible before release:
  - An ownership entry carries its own `order`, drawn from the plan clock and
    past every entry its writer held for the Task. The plan said the order is
    its comment's stamp. A move hands the entry to its target before the
    comment arrives, and the entry has to sort the same there.
  - "Saw nothing" is written as the entry naming itself in `seen`, so an
    absent `seen` can mean "written before first-wins" and never lose.
  - First-wins applies without Linear or Asana only. Entries there carry
    neither `order` nor `seen`, as on main.
  - "Stops its run" is the Task interrupt `lf task interrupt` writes, plus a
    steer naming who claimed first. The import kills no process: a live Graph
    stops before its next step and ends its agent's turn.
  - After the exchange shows a teammate's claim, `lf task run` and
    `lf task claim` still take the Task over, as on main (their Sessions are
    told). Should `lf task run` refuse a Task someone else holds instead?
  - The wait before a claim is five seconds, no setting. Past it the exchange
    keeps running and the claim goes ahead.
  - Only `lf task claim` and placing a Task (`lf task run`, checkout) exchange
    first. `lf ship` and adopting a checkout claim without looking.
  - `lf task status` does not mark a claim that lost; it is in the log like
    any entry and names no owner.

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

- QA (2026-11-18), each reversible before release:
  - Clones no longer converge on one repository id. Each keeps its own and
    records the ids other Machines' refs name as locators of its checkout
    (`locate_repository`). Converging rebinding left every Task saved under
    the clone's first id out of `lf task list` (the list reads by the selected
    id, and `task_repository_is_fixed` forbids moving a Task). The plan's
    "made for review" item 3 said the clone takes the lower id.
  - The first `lf task list` or `lf repo tracker` in a clone the Machine has
    saved nothing in does one Git exchange (five seconds at most) before it
    reads. Without it a teammate's fresh clone showed an empty plan and
    `Plan: Git origin.` for good, since the server only fetches registered
    repositories. Desktop's reads of such a clone do not do this.
  - A save made by the CLI is pushed on the server's minute, not "a few
    seconds after a change": 38 s to push and 97 s to show on the other
    Machine, measured. A `task.create` over the socket pushed in 1.6 s.
    `docs/work.md` now says "within a couple of minutes". Should a CLI save
    ask the running server to push?
  - A second clone of one remote under the same `LF_HOME` shows an empty plan
    (its Task ids already belong to the first clone) and, with a hub, an error
    telling the person to `--bind`. Its own saves go out on the Machine's ref.
  - Right after an upgrade from the released `lf`, `lf repo tracker` reads
    `Plan: Git origin.` with nothing unsent although nothing is on the new ref
    yet; the first exchange publishes the whole plan.
  - A teammate's Machine reads as its id (`ada claimed this on home_69e1…`),
    in `lf task list` too. Main's rule (labels stay on their Machine), now
    seen by everyone who shares a remote.

- Concept review of the built branch (2026-11-18,
  `scratch/concept-review.html`, questions 1-5), reversible before release:
  - A CLI save pushes its records by Git itself, without a fetch, as it
    already sends them to the hub (plan slice 4). Passed over: asking the
    running server; leaving it on the server's minute.
  - Left as built and asked on the page: `lf task run` taking over a Task a
    teammate holds; `lf task sync` needing a Task id without a tracker; a
    teammate's Machine reading as its id; an id per clone.

# Open choices (43bf9b, kickoff, 2026-11-18, headless)

`scratch/plan.md` is now this Task's plan; everything above is LOO-938's.
Each is reversible before release.

- A new or moved Task joins its Wave's newest `started` Project, else its
  newest open one, else a Project made then and named for the Wave. No
  `--project` flag. Should several open Projects in a Wave ask which?
- `lf wave edit-project` goes with the other two, as the brief lists it. A
  Project's name, summary and description are then edited only in the
  tracker; without a tracker nothing edits them.
- The standup board header loses its Project line.
- Hub records decode per field, like Git's, so a Machine on LOO-938's build
  sending `krs` does not fail the page.
- `-- leaving:` marks want an ID like `LOO-123` (`check_migrations.py`
  `LEAVING_MARK`); this Task is `43bf9b`. Widen the pattern, or does the Task
  get a tracker ID first?
- `remove_chapters.sql` is a second unmerged migration on a stacked branch,
  made by hand because `new_migration.py` answers with the parent's
  `plan_hub.sql`. Not yet run through `check_migrations.py`.
- `project_changes` is marked leaving here though the brief does not name it
  (only a test reads it).
- Left alone: this repository's `.lf/wave/product/GOAL.md` "Chapter planning"
  section and `.lf/wave/intelligence/metrics/*.md`, which nothing will read.
- `~/.lf/chapter-locks` is renamed `wave-locks`; a lock held by a running old
  `lf` during the swap is not seen by the new one.

- Concept review (43bf9b, 2026-11-18, headless, `scratch/concept-review.html`,
  questions 1-5), each reversible before release:
  - The "newest started" rule stays, and creation and move name the Project
    joined when the Wave has several open. On one store it answers the old
    current Project in 17 of 17 Waves; "oldest started" in 15.
  - `uncreated` keeps its Project half and tests the Project's `plan_stamps`
    row instead of `project_transitions`; an unstamped Project counts as made
    here, so the released `lf`'s unexported Project is queued by the first
    drain. Passed over: queuing the create in the creating transaction, which
    misses Projects made before a tracker is connected.
  - Desktop's "Project history" becomes "Projects"; the CLI line drops
    "current" and "historical".
  - Left as planned and asked on the page: a Wave without a tracker still
    gets a Project row on its first Task; no command completes a Project
    without a tracker; KR text in saved Projects stops being shown;
    `lf wave edit-project` goes.

## 43bf9b write-docs (2026-11-18)

- Without a tracker, a Wave keeps the one Project its first Task made: no `lf`
  command renames, adds or ends a Project once `lf wave edit-project` goes.
  `docs/work.md` "Projects" states this. Wanted, or does a local rename stay?

## 43bf9b implement, slice 2 (2026-11-18)

- A new Task's Project is now chosen and, when the Wave has none, made inside
  the same transaction as the Task's save or move (`join_project_in`), so a
  failed save leaves no orphan Project for the drain to export.
- Seen while testing it: with a `RAISE(ABORT)` trigger on `tasks` or
  `task_creation_intents`, `lf task create --wave` and `lf task move --wave`
  report "migration history or schema differs ... last migrated by lf
  0.16.0 (91038e53d)" instead of the abort; `lf task list` and `lf task edit`
  on the same store report normally. Some step on the Wave path reopens the
  store and validates its schema after the failure. Not chased here; the
  rollback is covered by `store::ownership::tests::a_failed_save_into_an_empty_wave_makes_no_project`.
