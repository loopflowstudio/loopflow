# Work with children

Plan, 2026-11-18, for Task c6a874. Exact words, earlier drafts and what the
probes showed: `scratch/said.md`. Choices made here with nobody present are
marked **made for review**; the still-open ones are in `scratch/questions.md`.
`scratch/explore.html` predates this direction.

## What to build

A Task can have children, and the conversation that filed them keeps
conducting them: it hears when one stops, fails or lands, and it stays open
until it is closed.

## The problem, as the code has it

Managing follow-ups from a Task's conversation is the pattern in use. Reading
the code moved where the lifecycle fights it:

- Follow-ups record no link back. `lf task create` ignores its caller on
  purpose (`ops/task/adoption.rs:71`: "Plain creation never adopts the
  caller's checkout or conversation").
- A conversation only hears about its own Task. The Machine server already
  wakes an idle conversation with "Task updates" (`server/wake.rs`,
  `harness/task_updates.rs`), but membership is `session_tasks()`
  (`sqlite/task_work.rs:66`): the Session's Task and nothing below it. A PR
  merge is not in the update at all.
- Merge does not finish a Task. Merged (`pr.phase`) and done
  (`tasks.planning_completed`) are already two facts, and the checkout is
  retired 15 minutes after completion, not after merge
  (`sqlite/task_retirement.rs:219`). What pushes a merged parent closed is the
  nudge: `next` is `lf task complete <ID>` and the reason reads "Merged;
  complete the Task" (`waves.rs:2134`, `task_row.rs:194`), and Auto-Ship
  completes at merge when no interactive step is left (`ops/task/ship.rs:496`).
- Once it is closed, the conversation cannot come back. Retirement leaves
  every Session row with a `cwd` that no longer exists; resume always launches
  in `session.cwd` (`agent/session.rs:91`, `harness/native_session.rs:182`),
  and a primary Session's repair path refuses: "Task … is done"
  (`ops/task.rs:1154`).

## The demo

In LOO-937's conversation: "file the follow-ups." `lf task status LOO-937`
lists three children with each one's step. LOO-937's PR merges: its row reads
merged, three children open, and nothing tells anyone to complete it. A child
lands and the conversation says so without being asked; another stops at its
review and the conversation brings it up. `lf task complete LOO-937` closes
the children with nothing unmerged and names the one that still has an open
PR. After LOO-937 closes and its checkout is gone, `lf resume` reopens the
same conversation in the repository's main checkout.

## Decisions

**1. A conversation whose directory is gone opens in the repository's main
checkout.** One resolution at launch: a recorded `cwd` that is not a directory
becomes the Task's repository root, and the Session row takes the new `cwd`.
A done Task's primary stops calling `task_checkout` to rebuild a checkout it
no longer owns. This drops a precondition; nothing new refuses.

- Probe (Claude Code 2.1.328, `claude -p`): a conversation started in
  directory A resumed with `--resume <id>` from B and from C after A was
  deleted, recalled its content, and ran tools in the new directory. The
  transcript stayed under A's encoded project directory.
- Probe (codex-cli 0.170.0, `codex exec`): a thread started in A resumed with
  `codex exec resume <id>` from B after A was deleted; `pwd` gave B.
- Not probed: the interactive `claude --resume` terminal launch and Codex
  app-server `thread/resume` with a new `cwd`. Slice 1's test covers both
  through lf's own launch path.
- lf's own transcript reads find a file by id from any cwd
  (`transcript::find`, `transcript/mod.rs:257`), so nothing is copied.

**2. The checkout still goes at close, not at merge (made for review).** A
merged parent keeps its checkout while it is open. Passed over: retiring at
merge and moving the live conversation to the main checkout, which would pull
the directory out from under a running provider process and its children.
The cost: the conductor reads a branch that stops moving once it merges.

**3. One link: `tasks.parent_task_id`, set at filing.** `lf task create` run
from a Task's conversation files a child. The caller's Task comes from
`conversation_task(store, caller_session)` (`ops/human_session/moves.rs:161`)
with the Session taken from `LF_AGENT_CALLER`. Flat by default: when the
calling Task itself has a parent, the new Task takes that same parent and is
its sibling. `--parent <ID>` says otherwise; `--parent none` files top-level.
Outside a Task conversation nothing changes. A cycle is refused the way
stacking's is (`sqlite/children.rs:880`). Parent and stack stay two facts:
`--stack-on` and `lf sync <ID>` are untouched.

**4. A child takes its parent's Project when it is filed (made for review).**
With no `--wave`, `project_id` is copied from the parent, so the child is in
the parent's Wave through the join that exists. Linear does the same: "If
omitted for a sub-issue, the parent's project is used" (`IssueCreateInput`,
linear/linear `schema.graphql`, read 2026-11-18). Passed over: finding the
Project by climbing at read time, which rewrites `task_wave_in`,
`TASK_COLUMNS` and seven more `p.id=t.project_id` joins while LOO-938 is
rewriting the same records, and leaves `agent_sessions.wave_id` stale when a
parent moves. Climbing belongs with the later Wave and Task fusion.

**5. The parent travels as a plan field and as the tracker's own parent.**
`WorkValue::TaskParent` beside `TaskProject`. Linear: `parentId` on
`issueCreate` and `issueUpdate`, `parent { id }` in `issue_fields!`
(confirmed in Linear's published schema). Asana: `parent` on `POST /tasks`
and `opt_fields` (from memory of the API; not called). No Loopflow-only field
lands on a tracker item. This is the part that overlaps LOO-938 (PR #1971,
open, Graph mid-implement): `planning_write.rs`, `work/edit.rs`,
`tracker_actions.rs`, and its new `plan_carriers.rs:125` `needs()`, which
must carry a parent before its child. It is built last, on LOO-938 via
`lf sync LOO-938` if that has not merged. LOO-938's plan says "Not built:
skipping a field this build does not know", so a peer on an older build may
fail on `task_parent`; with one person on current builds that is accepted.

**6. A parent's conversation hears about its children through the updates
that exist.** No new channel. `session_tasks()` gains the open children of
the Session's Task, and `TaskObservation` (`task_updates.rs:56`) gains the
PR phase and whether the Task closed, so "landed" is news. The update names
the child and reads as news about work this conversation conducts; today's
line says "not a request or permission to launch work", which is wrong for a
parent. Delivery is unchanged: an idle Claude or Codex conversation is woken
by the server, a busy one hears at its next tool or prompt hook, a closed one
at its next resume. A missed wake is already logged and the news waits
(`wake.rs:153`).

- New triggers advance `store_revisions.task_updates` when a `task_prs` row
  merges and when a Task closes, is abandoned or changes parent; today the
  `tasks` trigger covers three other columns.
- Limit, stated not solved: lf learns of a merge only when one of its own
  processes reconciles the PR (`lf land`, Auto-Ship, `lf pr reconcile`).
  A child merged by hand on GitHub is news at the next reconcile.
- `task_session.md` gains the paragraph on conducting children; no other
  skill learns it.

**7. Merged and closed stay the two fields they are.** `pr.phase == "merged"`
and `state == "done"` already say it; no `merged` or `closed` booleans are
added. What changes for a Task with open children:

- Its reason reads "Merged · 3 children open" and `next` is null.
- Auto-Ship's merge does not request completion (`ship.rs:496`).
- `lf task complete <parent>` names the open children, then closes with it
  every child that has nothing unmerged (no PR, or a merged one, and no run).
  A child with an open PR, a live run or unpublished commits stops the
  command, which lists them and takes `--keep <ID>` (the child moves up to
  the next parent, or top-level) or `--abandon <ID>` (made for review).
  Jack Heart: "3: i think probably close by default". Passed over: abandoning
  unmerged children without being told, since abandonment force-removes the
  worktree and deletes the remote branch with no grace period.

**8. `--parent` reads the caller; `lf task create` help says so.** The line
"Plain creation files new work, regardless of the current checkout or
invoking Session" (`lf/mod.rs:2244`) and the adoption comment go.

## Source of truth

`tasks.parent_task_id`. Children are the Tasks that name a parent, ordered by
filing time; no stored child list. Open, merged and closed derive as today
(`task_state_sql`, `sqlite/durable.rs:346`).

## Surfaces

| Surface | Change |
|---|---|
| `lf task create` | `--parent <ID\|none>`; default from the calling conversation; receipt carries `parent` |
| `lf task status --json` (`task_row::TaskDetail`) | `parent`, `children: [{identifier, title, state, step, pr}]` |
| `lf task show` (`TaskStatus`, `DesktopTaskPane`) | same two keys; human output lists children |
| `RoadmapTask` (lists, server, Swift mirror) | `parent`, required and nullable, so slice 4 has something to draw |
| `lf wave show` | a child prints indented under its parent |
| `lf task complete` | closes children; `--keep`, `--abandon` |
| served `task.create` | `parent` |
| Task updates | children's position, runs, merge and close |
| Fixtures | `task_detail.json`, `task_status.json`, `roadmap_snapshot.json`, `task_create_receipt.json`, `local_task.json`, `server_schema.json`, Swift `DTOFixtureTests` |

Absent and error states: no parent is `null` and no children is `[]`, never a
missing key. A parent that was deleted or is unknown to this Machine leaves
the child top-level with its link kept. `--parent` naming an unknown Task is
an error naming it. A conversation with no Task files top-level, as today.

## Operational limits

- `session_tasks()` runs on every hook boundary of every agent. The children
  lookup is one indexed read on `tasks(parent_task_id)`; check
  `EXPLAIN QUERY PLAN` against a 10,000-Task store.
- Updates for children are written inside the reading Session's hook, as
  today; no new writer, no new lock holder.

## Exclusions

- The sidebar (slice 4): played with in the running app once children exist.
- Wave and repository primary conversations hearing about Tasks; they have no
  member Tasks today and gain none here.
- Dependencies ("start this when that lands"), the doc store, plan edits
  reaching children, one way to ship, the chapter cut: their own Tasks.
- Wave and Task as one stored type; Project by climbing.
- A server-side watcher for merges made outside lf.

## What goes

- `task_prs.after_merge`, `task_prs.next_slug` and the same pair on
  `pr_landings`: no writer, and `map_task_pr_row` skips them
  (`sqlite/children.rs:1812`, `2372`). Out of `TASK_PR_COLUMNS`; marked
  `-- leaving:` in this Task's migration and in `SCHEMA.md`.
- The "Merged; complete the Task" nudge for a Task with open children.
- The creation rule that ignores the calling conversation, and its help text.
- The done-Task refusal on the resume path (`validate_preparation_status`
  stays for `lf task run`).

Read and found live, so it stays here: several PRs in a row on one Task.
`task_prs.sequence`, `historical`, `request_task_pr_publication`
(`ops/task/delivery.rs:120`), `docs/ops.md:38` and eight
`continued_publication_*` tests (`tests/task_pr_range_tests.rs:1038`) are
shipped behaviour from v0.13.16. "At most one PR per piece of work" removes
it, and `lf land` after new commits then files a child that takes the branch.
That is a behaviour change with its own tests: a follow-up Task filed from
this Task's conversation once children exist, not part of this PR. The seed
sentence "A Task has zero or one pull request" (`ops/task_input.rs:170`)
already describes that end state and is left alone.

Survives: Task ids, worktrees and PRs; stacking; retirement timing; every
existing update a conversation gets about its own Task; Waves and
`.lf/wave/**`.

## Slices

1. **A closed Task's conversation resumes.** Launch resolves a missing `cwd`
   to the repository's main checkout for both the bound and the primary
   Session, Claude and Codex. Shown by: complete a Task, run `lf task retire`,
   `lf -b resume <session> --message …` answers from the main checkout. This
   is the one that could prove the plan wrong; the provider half is already
   answered by the probes, so what it tests is lf's launch path (config and
   skills loaded from `session.cwd`, `journal::with_runtime(cwd)`,
   `current_dir`). Nothing here touches LOO-938's files.
2. **The link, local.** Migration (`parent_task_id`, its index, the leaving
   marks, the new triggers), `lf task create` default and `--parent`, Project
   copied, `parent` and `children` on status, show and `RoadmapTask`.
3. **Hearing about children.** Membership and the two new observation facts;
   the child wording; `task_session.md`. Test: a fixture child Graph stops at
   an interactive step and the parent's idle conversation receives the update.
4. **Merged and open; closing.** Reason and `next` for a parent with open
   children, Auto-Ship's skipped completion, `lf task complete` over children.
5. **The link travels.** `TaskParent` through the plan exchange and to Linear
   and Asana, on LOO-938.

One PR. If LOO-938 is still unmerged when slices 1–4 are done, slice 5 is
built on `lf sync LOO-938` and the PR waits behind #1971.

## Acceptance check at gate

A fixture test, headless, with a `MachineServer`:

```bash
lf task create 'follow-up' --json             # from LOO-A's conversation: .parent.identifier == "LOO-A"
lf task status LOO-A --json                   # .children[0].identifier is the follow-up
# LOO-A's PR merges
lf task status LOO-A --json                   # .task.pr.phase "merged", .task.state not "done", .task.next null
# the follow-up's Graph stops at an interactive step
lf session steers <LOO-A's conversation>      # an update naming the follow-up and its step
lf task complete LOO-A                        # closes LOO-A and the follow-up
lf task retire && lf -b resume <LOO-A's conversation> --message 'pwd'   # answers from the main checkout
```

## Done when

From a Task's conversation, a follow-up is filed with no flag and shows under
that Task; the conversation is told when the follow-up stops, fails or lands;
the Task's PR can merge and the Task stay open without being told to
complete; closing it closes its children or says which ones it will not; and
its conversation reopens after its checkout is gone.

## The other designs, intent only

- **Dependencies.** "Start this when that lands", set by the parent. Should
  not add a second way to say "follow-up of".
- **Doc store.** A repository setting beside the tracker: Notion, Google Docs,
  or a Git ref by default. No CRDT: Git's text merge, a conflict shown when
  it fails.
- **Plan edits reach children.** Each child reads the latest plan at its next
  step; the parent's conversation reads the change and steers the children it
  touches.
- **One way to ship.** `lf ship` becomes shorthand, not its own state.
- **Chapter cut.** Chapters, KRs and metrics go; after LOO-938.
- **One PR per Task.** Serial PRs go; `lf land` after new commits files a
  child. Found while reading for this plan.
