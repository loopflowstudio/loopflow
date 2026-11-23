# One tree of Waves: the design as it would be built

2026-11-20. One agent's best case, written for review. Nothing here is
decided unless it says **said**; those lines have their words in
`scratch/said.md` and their firmness in `scratch/wave-tree.md`. Everything
else is a recommendation. Tracker facts: `scratch/hierarchy-research.md`.
The earlier plan with the full rule list: `scratch/plan.md`.

## The problem, as it happens

Jack Heart, 2026-11-20: "I have been starting with a task, finding its kinda
big, and then deciding to launch more tasks. none of that is not sent to the
tracker."

So the follow-ups are real Tasks, filed in Linear, each with a PR. What is
missing today:

- a follow-up has no link back to the Task it came from;
- the thread that filed it hears nothing when it stops, fails or lands;
- the first Task is pushed to done when its own PR merges, with work still
  open that came from it;
- once done, its conversation can't be picked up again.

A second problem sits beside it: a Task that grew has to turn into a
different kind of thing (a Wave directory committed to main, a Linear
Project) to hold work, and that changes its ID, its thread and where its
notes live.

Hiding one person's fan-out from a team's tracker is a third problem. It
belongs to someone inside a large organisation, nobody using Loopflow today
has it, and an earlier draft of the plan wrongly built slice 1 around it.

## What to build

One kind of work, a Wave, in a tree that Loopflow owns. Any Wave can hold
other Waves. Some have a person working on them, with a thread, a worktree
and PRs. Some are connected to a Linear Project or issue. A Task that grows
keeps its ID, thread and worktree and simply has Waves under it.

## The record

The single kind grows from the Task record (**said**: "1) Agreed").

```
Wave
  id, title
  parent        one Wave, or none; no loops
  open          closed and reopened cheaply
  shared        false = only its author's
  tracker       at most one: a Linear Project or a Linear issue
  chat, docs    optional connections
  goal, memory, settings   optional; read by going up

WaveHome        one per person per Wave, and only if they work on it
  machine       the session and the worker are both here
  session       one chat session
  worktree      one; at most one active worker in it
  branch        one remote branch; PRs over time, one current
  graph step    where the work stands
```

The test for "one kind" is one scenario: a Task with a merged PR gets five
Waves under it, then a goal and memory, then a Linear Project, and at no
point does its ID or thread change. If that scenario needs a conversion
step anywhere, the kinds are still two.

The risk in it: a group nobody works on (never ends, has memory and a
cadence) and a piece of work (ends, has a PR) share only `parent`, `title`
and `open`. One table will carry many empty columns. That is acceptable if
the empty columns are whole optional parts (`WaveHome` absent, goal absent),
and a smell if code starts asking "is this Wave really a Task?".

## The recommendations

**1. A new Wave is shared if the Wave above it is shared.** Filing from a
Task's thread makes a filed follow-up, as today. Everything under a personal
Wave is personal. "Fan-out must not require a public Wave at every level"
(**said**) is met by a flag at creation, `--personal`, and by the rule that
a shared Wave can hold personal ones and never the reverse (**said**).

**2. The person types what they type today.** `lf task create '<outcome>'`
from a Wave's thread or worktree puts the new one under that Wave, because
the session already knows which Wave it is. `--parent <id>` names another;
`--parent none` puts it at the top. No new verb for fan-out.

**3. A merge does not finish a Wave with open Waves under it, and a Wave
someone works on finishes itself when its current PR is merged and nothing
under it is open.** The thread is told. A group nobody works on never
finishes itself. Reopening is one command and brings the thread back.

**4. A thread hears the level directly under it.** Each level tells the one
above; a thread three levels up reads a summary, never the raw events.

**5. Loopflow never writes "Fixes LOO-123" in a PR.** Every PR says "Part
of LOO-123", and Loopflow sets the issue to done when the Wave closes. One
rule covers one PR and ten, and nobody has to know which PR is the last.
Cost: a PR merged by hand with Loopflow off leaves the issue open until
`lf` next looks. Not checked: Linear's exact non-closing words, and whether
a team's "on merge, move to done" automation fires for a PR linked this way
or by branch name.

**6. In Linear, a Wave under a connected Project is an issue in that
Project, and nesting is carried by sub-issues only as far as it has to be.**
See the next section; this one changed while writing.

**7. A connected Wave's title, status and assignee are the tracker's.**
Parent, shared, home, goal, memory and settings are Loopflow's. Two owners
for one field is where sync bugs come from.

**8. One tracker per repository.** Jack Heart: "ends up being kind of
confusing". A tree that mixes Linear and Asana by branch has no single
answer to "where is this filed". Chat and docs can vary by Wave.

**9. Keep the word Task for now.** Slice 1 changes no command names. The
rename question (`lf task` and `lf wave` as one noun) is decided when
today's Waves become the same record, where it stops being cosmetic.

## The link has to travel somewhere

With Linear connected, Linear carries the plan between Machines; Git and the
hub carry it only for a repository with no tracker (`docs/work.md`, "Share
the plan"). So a fact that is not in Linear does not reach the other
Machine. A parent link kept only in Loopflow is such a fact: file three
follow-ups on the laptop, and on the mini they are three unrelated issues.
Not confirmed in code; if Loopflow-only Task facts already travel by another
path with Linear connected, this section is moot.

Three ways out:

| | What carries the parent link | Cost |
|---|---|---|
| a | Linear sub-issues | Linear copies team and Project to a sub-issue once and never reconciles; a team's auto-close switches act on parents and children; moving a Wave in Loopflow now edits Linear. This is what #1986 built. |
| b | The Git plan ref, always on, for facts the tracker does not hold | Two carriers at once for one plan, each owning different fields. Small if it is only `parent`, `shared` and home. |
| c | A line Loopflow writes in the issue ("From LOO-123") and reads back | Text in a description as data. Anyone can edit it. |

Recommendation: **b**, with **c**'s line written once as a courtesy to
people reading Linear, never read back. It keeps rule 7 true (the tree is
Loopflow's, the tracker owns three fields), it is the first small piece of
"the Git Wave store always" (leaning, **said** as a question), and it is
the same path personal Waves need later, since those can never travel
through Linear. It moves that piece from slice 5 to slice 1.

The case for **a** is real and is the cheapest today: it is built, and it
shows nesting to someone reading Linear without Loopflow. It fails the
moment a Wave is personal or sits above a Project.

## Slices

1. **Work under a piece of work.** `parent` on the Task record; filing from
   a thread sets it; the thread hears; a merge doesn't finish a parent; a
   finished one reopens; the parent link travels by the Git plan ref.
   Follow-ups are filed in Linear as ordinary issues in the same Project.
   This is close to #1986 with the sub-issue write replaced by the ref.
   Start clean, with #1986 as reference (**said**).
2. **Groups in the same record.** Today's Waves become rows with nobody
   working on them. Their `GOAL.md` and `MEMORY.md` stay in `.lf/wave/`
   for now and are read by going up the tree. The rename is decided here.
3. **Personal and publish.** `--personal`, `lf wave publish`, and the
   one-way rule. Needs slice 1's ref, since personal Waves travel only
   there.
4. **Home Machine and steering across your Machines.**
5. **Memory out of the repository.** Needs the doc store (LOO-997).

Moving memory last is deliberate. In the last month 54 of 55 commits that
touched `.lf/wave/` were `MEMORY.md` edits: the tree almost never changes,
memory changes constantly. Memory is what wants out of main, the doc store
is what lets it leave, and nothing in slices 1 to 4 waits on it.

## Delete — do not maintain

- `tasks.parent_task_id` as #1986 wrote it and its sub-issue write to
  Linear (never merged).
- `tracker_wave_projects` and a Wave's Project list, once a Project is a
  connection on a Wave (slice 2).
- Nested Wave directories as the way to nest (slice 2); `.lf/wave/` itself
  when memory leaves (slice 5).
- Team and initiative links: `linear: initiative:`, `pm_wave_sync.initiative`,
  `project_initiatives`. Liveness not checked.

Must survive: a started Task's ID, worktree, thread and PR; every Wave's
goal and memory; every Linear issue already linked.

## Where this is weakest

- **The travel problem was found while writing this.** It rests on one
  reading of the docs. It decides whether slice 1 is small.
- **"Shared" does two jobs.** It means "teammates' Loopflow sees it" and,
  under a Project, "it is an issue". Above a Project it means only the
  first. That may want two words.
- **A person working directly on a Wave connected to a Project** has PRs
  with no issue to link. Allowed here, not thought through.
- **Closing a Wave with open Waves under it by hand.** Recommendation 3
  covers finishing by merge only.
- **Linear as the plan's carrier for some fields and Git for others** is
  the kind of split that produced "a little unconstrained". Option b adds
  to it before slice 5 removes it.
- Four levels of user were played through on paper
  (`scratch/wave-tree-dynamics.html`); only the first exists.

## Questions for review

1. Does a Loopflow-only fact reach a second Machine today when Linear is
   connected? If yes, how, and does the travel section fall away?
2. Parent link: (a) sub-issues, (b) the Git plan ref, (c) a line in the
   issue.
3. Is "shared if the Wave above is shared" right as the default?
4. "Part of" on every PR and Loopflow closes the issue: acceptable, or
   should a single-PR Wave keep "Fixes"?
5. Does slice 2 hold with goal and memory still in `.lf/wave/`, or does
   having rows in the store and files on main for one Wave cost more than
   waiting for the doc store?
6. Is there a scenario where one record for groups and work forces code to
   ask which it is?
