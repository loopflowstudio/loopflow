# One tree of Waves

Design direction, 2026-11-22. Earlier proposals are in `scratch/wave-design.md`;
the conversation is recorded in `scratch/said.md`.

## What to build

One nestable work record grows from today's Task. Task names an outcome to
deliver. Wave names a scope from which to direct work over time. A record
can do both, keep its ID and conversation as children join, and finish at
any depth. Creating work requires no choice between the two.

The parent identifies the conversation conducting the work. A prerequisite
identifies an outcome another piece of work needs. LOO-797 conducts LOO-796
(draft continuity), and LOO-798 also needs 796. Either conversation can see
what it depends on; only one conducts 796.

## How work appears in Linear

| What someone follows | Linear representation |
|---|---|
| A grouping used to direct work in Loopflow | No tracker item required |
| One deliverable, possibly across several PRs | Issue |
| A Task under another Task (both Linear issues) | Sub-issue (`Issue.parent`, synced two-way) |
| Independently shippable outcomes with a larger finish line | Project containing issues |

Each work record can link to one Linear issue or Project. When both parent and
child are Linear issues, `parent_task_id` syncs with Linear's `Issue.parent`
(agreed 2026-11-22: Option A). Linear requires no Project or Team alignment
between parent and sub-issue, and its auto-close switches default off. When
non-issue parents arrive in Slice 2 (Waves or Projects), those links stay in
Loopflow and travel on `plan_carriers`.

## The problem

Work fans out. One thread ends up driving follow-ups, smaller designs and
sequences of releases, and today each of those has to become a different
kind of thing: a second Task with no link back, a plan doc checked in, or a
Wave committed to main and shown to everyone. Seen from the other side, a
team's tracker should show the work the team cares about and none of one
person's fan-out.

## The demo

Solo, no tracker. In the repository's thread: "build billing." A Wave
appears under the repository with a worktree and a PR. In billing's thread:
"split this up." Five Waves appear under billing. The first PR merges and
billing stays open. A child lands and billing's thread says so. Later:
"group these under infrastructure", and the tree changes with no commit.

Team on Linear. `lf wave connect store-rework linear:project/store-rework`,
`lf wave publish store-rework`. "Add these three to store rework" files
three issues in that Project. One person takes an issue and splits it into
four; Linear still shows one issue and its PRs.

## The model

**Wave.** The only kind of work.

| Field | Meaning |
|---|---|
| parent | one other Wave, or none at the root; no loops |
| shared | personal until published |
| open | closed and reopened cheaply |
| goal, memory, settings | each optional; read by going up |
| connections | at most one tracker item; a chat; docs |

**A person's place on a Wave** (`WaveHome`, **proposed** name). At most one
per person per Wave; a person need not have one.

| Field | Meaning |
|---|---|
| machine | the home Machine: the session and the worker are both on it |
| session | one chat session |
| worktree | one, on the home Machine; at most one active worker in it |
| branch, PRs | one remote branch; PRs over time, one current |

Cron jobs are per Machine and run in a worktree of their own. A shared Wave
has no cron of its own without a hosted server.

**Connection.** A Wave to one outside thing: a Linear Project, a Linear
issue, a chat channel, a doc or folder. Not a Linear team or initiative.
Filing needs a team; it is a setting found by going up.

## Rules

1. When both parent and child are Linear issues, `parent_task_id` syncs two-way
   with Linear's `Issue.parent`. Non-issue parents (Slice 2+) stay in Loopflow
   only.
2. What a Wave lacks it finds by going up. With none up to the repository,
   the action fails and says what was missing.
3. A shared Wave can contain personal Waves; a personal Wave cannot contain
   a shared one. Publishing shares one Wave.
4. Work filed in Linear uses the mapping above.
5. A new Wave under a shared Wave with no worker is shared; under a Wave
   someone is working on, personal. (**proposed**, unanswered)
6. A connected Wave's title, status and assignee are the tracker's.
   Everything else is Loopflow's. (**proposed**, unanswered)
7. Nothing is created outside except by publishing under a Project or by
   asking. No importer: an agent builds the tree with the commands.
8. A thread hears when a Wave directly under it stops, fails or lands.
9. A merge does not finish a Wave that has open Waves under it.
10. Agents are not shared between people. Your own steers reach a Wave's
    home Machine by Loopflow's path; other people steer through the tracker,
    such as a comment on the issue.

## Commands

All **proposed**.

```bash
lf wave create store --parent infrastructure   # personal, connected to nothing
lf wave move store --parent product            # tree only
lf wave connect store linear:project/store-rework
lf wave connect infrastructure slack:#infra
lf wave publish store                          # this one becomes shared
lf wave show store                             # what it has, and what it gets from above
lf wave close store
lf steer store 'use the staged drop'           # reaches its home Machine
```

## Where it is stored

Leaning, not decided:

- Waves are not repository content. The shared store travels like the plan,
  over a Git ref, and each person's store is the shared store plus their own
  Waves.
- Linear and Asana stop carrying the plan; they are what Waves connect to.
- One person's hub is open source. Anything several people's Loopflow talks
  to (a shared hub, shared crons, groups, permissions, a root above
  repositories) is for a hosted service, behind one seam.

## Constraints

- **The single kind grows from the Task record.** Agreed 2026-11-20. It already has a person on a Machine, a worktree, a Graph, PRs
  over time and Sessions, lives in the store and travels live. Today's Waves
  become rows of the same kind with nobody working on them, and their files
  on main stop being read. Growing from the Wave record instead means
  teaching a directory on main to be cheap and live.
- **Memory out of the repository** needs somewhere to live and a way for an
  agent to read and write it. This is the doc store (LOO-997), so it comes
  before Waves leave the repository.
- **Released `lf` reads `.lf/wave/` and `.lf/config.yaml`.** The shared-file
  rule applies while both shapes exist.
- **A Wave open while its issue closes.** Loopflow does not write `"Fixes"` in
  PR bodies (`ops/pr.rs` uses a `Task:` link plus Linear attachments); verify
  whether Linear's GitHub branch-name integration closes the issue on merge.
- **A personal Wave that needs a person** must not post where a team reads.
  Going up to post has to stop at the line between personal and shared, or
  find "where to reach me" separately. Unanswered.

## Delete — do not maintain

- PR #1986 as a branch (`jack-heart/subwaves2` is closed, never merged, kept
  only as reference; Slice 1 rebuilds cleanly off `main`).
- Task and Wave as two kinds; Project as a stored level between them
  (`tracker_wave_projects`, a Wave's Project list).
- `.lf/wave/<name>/` on main: `GOAL.md`, `MEMORY.md`, `config.yaml`, and
  nested Wave directories.
- Linear and Asana as the plan's carrier.
- Team and initiative links: `linear: initiative:` in Wave config,
  `pm_wave_sync.initiative`, `project_initiatives`. Liveness not checked.
- KRs, metric targets, activation and Project rollover (PR #1997, open).

Must survive: a started piece of work keeps its identity, worktree and PR;
existing Waves' goal and memory; every Linear issue already linked to a Task.

Slice 1 starts clean; #1986's branch (`jack-heart/subwaves2`) is a reference
to read, not a base. Worth rebuilding from it: `tasks.parent_task_id` with
two-way Linear sub-issue sync, a thread hearing about direct children, a merge
not finishing a parent with open children, and reopening a closed conversation
from the main checkout.

## Slices

An indivisible direction built as an additive series. Each runs end to end.

1. **Conduct children from an existing conversation (Slice 1 — ready to test-drive).**
   Clean branch off `main`, using `jack-heart/subwaves2` as reference.
   - **Data:** `tasks.parent_task_id` (same repository, cycle-checked), synced
     two-way with Linear `Issue.parent` when both are Linear issues, and carried
     by `plan_carriers` on trackerless repos.
   - **CLI:** `lf task create '<outcome>'` stays top-level by default;
     `lf task create --parent <id> '<outcome>'` creates a child (never
     auto-inferred from the current thread or worktree).
     `lf task update <id> --parent <parent|none>` adopts or unparents.
     `lf task reopen <id>` reopens a done Task's conversation from the main
     checkout. `lf task status` shows parent and direct children.
   - **Conducting loop:** parent session receives compact updates when a direct
     child stops at an interactive step, fails, merges, or completes; merging a
     parent's PR settles the PR without completing the Task while open children
     remain.
   - **Done when:** in a Linear-connected repo, create parent + two `--parent`
     children (visible as sub-issues in Linear and on a second Machine), merge
     the parent's PR while children are open (parent stays open and shows open
     children), land/fail a child (parent session receives the update), complete
     both children (parent can complete), and `lf task reopen` brings the parent
     conversation back.
2. **Groupings in the store.** Today's Waves become the same kind. Goal,
   memory and settings are read by going up. Needs filtered `plan_carriers` for
   non-issue parents; `.lf/wave/` files can stay until the doc store (LOO-997).
3. **Connect and publish.** `--personal`, `lf wave publish`, and the one-way rule.
4. **A home Machine,** inherited from the Wave above, and steering that
   reaches it.
5. **The Git Wave store always,** with Linear as a connection only, and memory
   out of `.lf/wave/` (LOO-997).

Separate designs, already filed: starting work when other work lands
(LOO-998), the doc store (LOO-997), plan edits reaching the work under them
(LOO-999), a group's defaults (LOO-1000, absorbed by rule 2).

## Done when

See Slice 1 above. Each later slice names its own command and result after the
Slice 1 test drive.
