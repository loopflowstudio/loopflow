# Chapters — the In Progress Projects

2026-09-27 · LOO-298 · Design for Cut H7, rewritten after review with Jack and
accepted by him "for now" the same day.
Supersedes the same-day draft that added a `chapters` table, a `chapter.json`
plan file and a config line. Jack's direction: the current Chapter code was
expedient prototyping, not a blessed model; deleting more is better; one
deployment (his) is primary, but the shape must not be a dead end for several
Homes; chapter history and current ids need not be preserved; existing Linear
Projects must not be thrown away; a developer who has not pulled must still see
both chapters in Linear and be able to detect and adopt the new one; "newest
wins" by name should not decide anything.

## The model

A Chapter is not a stored object. It is the set of Projects that are In
Progress in Linear, one per Wave. Their shared name (`2026-10`) is the title
Loopflow gives Projects it creates together, so a person can see they belong
together. History is the Completed Projects. Nothing else exists.

```text
Linear Project (per Wave)     status: Planned | In Progress | Completed | Canceled
  content lines               flow: feature            the Flow its Tasks run
                              KR / target lines        unchanged
  Tasks (issues)              unchanged

local projects row            + status  (synced; no other new column)
```

- A Wave's current Project is its one In Progress Project. Its Tasks start
  there; `lf task run` runs the Project's Flow unless `--flow` overrides.
- A Planned Project with a future name is a Wave's plan for the next chapter.
  Anyone creates it in Linear whenever they like. That is the bottom-up part.
- Completed Projects stay visible in Linear and keep their Tasks until they
  drain; `lf runs --project` and the Task pages still read them.
- The repository has one chapter when every Wave's In Progress Project shares a
  name. When they do not, `lf status` names each Wave's current Project and
  `new-chapter` refuses until a person resolves it. Nothing wins silently.

## Usage

```sh
lf repo new-chapter 2026-10 --dry-run   # every Wave: successor, Task dispositions
lf repo new-chapter 2026-10             # create, transfer, flip statuses
lf pm show --wave infrastructure        # the Wave's In Progress Project and Tasks
lf task run INF-123                     # the Project's Flow
```

`lf repo` is a new command family: rotation addresses every Wave in the
repository, and `lf wave` addresses one. It starts with this one command;
`lf pm init --all` and `lf roadmap` are its natural later neighbors, not moved
here. `new-chapter` takes no plan file. For each Wave in the repository: use the
Wave's Planned Project named `2026-10` if one exists, else create an empty one
(title from the Wave's existing naming, `flow:` copied from the current Project);
classify the current Project's Tasks with the existing disposition code, move
started unfinished Tasks to the successor and cancel untouched backlog; set the
successor In Progress and the predecessor Completed. Each step is retryable by
running the command again with the same name. No SQLite transaction spans it
and none is claimed: Linear is the owner and the command converges on it.

Another Home detects the change on its next `lf pm sync`, which every planning
read already triggers: its `projects` rows now carry the new statuses, its
Wave's current Project has changed, and `lf status` says so. Its local Tasks
already point at the right Project because Linear moved the issues. There is
no switch command because there is nothing local to switch.

## What changes in code

Add: Project `status { type }` to the Linear projects query and `PmProject`;
`projects.status` on sync; `ops/chapter.rs::rotate` over every Wave using
status instead of the receipt phases; `flow:` replaces `recommended:` as the
content key, `ProjectFlowPlan { recommended: Option }` becomes `Project.flow:
String` (required at creation; the Swift mirror follows).

Delete: `wave_chapters` and `store/sqlite/chapters.rs` as a chapter store;
`work/chapter.rs` (`Chapter`, `ChapterId`, `ChapterPhase`, `ChapterMetricEvidence`;
`TaskDisposition` and `ChapterTask` move next to the classifier that uses them);
`read_chapter`, `frozen_chapter`, `chapter_history`, the `legacy-<project>` ids,
`ensure_successor`'s receipt bookkeeping; `lf wave history` and
`lf status --chapter`; the Swift `ChapterSummary`, `chapterUnavailable` and
their fixtures; the `.lf/chapters/` packet directory and the code that reads or
writes it; `check_architecture.py`'s `wave_chapters` owner. Retirement keeps its
evidence checks (authored work, PRs, claims, `tasks.started_at`).

Migration: none of history. A draft drops `wave_chapters`. Existing Projects
stay as they are; the first `new-chapter` after this lands Completes them and
creates the successors. Until then each Wave's non-archived Project is treated
as In Progress.

## Not a dead end for several Homes

Linear is already shared. Two Homes read the same statuses; the only race is two
people running `new-chapter` with different names, which leaves two In Progress
Projects per Wave, visible, named by `lf status`, and fixed by completing one.
If a chapter ever needs to be a first-class object again, the Projects it groups
are all still there.

## Done when

`lf repo new-chapter --dry-run` lists every Wave with its successor and Task
dispositions; the real run leaves each Wave with one In Progress Project named
`2026-10` and its predecessor Completed with only finished or canceled Tasks;
started Tasks kept identity, worktree, PR and invocation; a second Home (private
`LF_HOME`) shows the new chapter after `lf pm sync` with no other action;
`lf task run` with no `--flow` runs the Project's `flow:`; `rg
"wave_chapters|ChapterId|\.lf/chapters" rust swift scripts docs` returns only
released migrations; `check_architecture.py` reports no missing owner.

## Open

- Whether a Wave with no In Progress Project (all Completed, none Planned) may
  start Tasks. Proposed: `lf task start` creates the Project for the current
  chapter name on the spot, so a Wave is never blocked on planning ceremony.
- Alternative to status considered: archived-vs-live, the convention the
  prototype already uses. It needs no query change but hides past Projects by
  default in Linear and cannot express Planned. Status was chosen for the
  visibility Jack asked for.

## Implementation review: partial rotation (2026-09-28)

Supervisor finding, not an additional decision by Jack: the unconditional
mixed-name refusal above conflicts with retry after a partial multi-Wave
rotation. If Wave A has reached the requested name while Wave B still has the
predecessor name, repeating the same command must be able to finish. Status
writes within one Wave can also temporarily leave zero or two In Progress
Projects, depending on write order. The existing implementation uses saved
Chapter phases to recognize continuation; deleting those phases requires
recognizing the allowed intermediate states from fresh provider evidence.

Implement and prove the distinction before removing the old recovery path.
A candidate rule uses the explicit requested name, stable Project identities,
and one unambiguous predecessor group to distinguish convergence from unrelated
conflicting plans. This is an implementation proposal to validate, not permission
to select whichever Project has the newest name. Interrupt after each provider
mutation, repeat the same command, and check both same-Home and second-Home
recovery. Retain a counterexample with unrelated competing current Projects
that remains unresolved. Do not add a Chapter table or packet to recover the
discarded prototype's phases.
