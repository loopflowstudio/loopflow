# Work with children

Plan, 2026-11-18, for Task c6a874. Exact words, earlier drafts and what the
probes showed: `scratch/said.md`. Choices made here with nobody present are
marked **made for review**; the still-open ones are in `scratch/questions.md`.
`scratch/explore.html` predates this direction.

## What to build

A Task can have children, and its conversation conducts them: it hears when
one stops, fails or lands, and it stays open until it is closed.

## The model

What a person knows, and the one record behind each. The page
`scratch/concept-review.html` draws it.

- **A Task** is one outcome with at most one PR. It may have a **parent**,
  another Task: the work it was filed from. That is the whole hierarchy.
- **A conversation belongs to one Task.** It works in that Task's checkout and
  direction for the Task is for it. This is `session_tasks()` and it stays
  exactly as it is: ten call sites rely on a Session belonging to at most one
  Task.
- **A conversation conducts the children of its Task.** A second relation,
  derived from the parent link and nothing else. It decides one thing: which
  conversations are told when a child's standing moves.
- **Merged** is a fact about the PR; **closed** is someone saying the Task is
  finished. Closing never takes the conversation away and never moves a child.
- **Stack** says which branch a Task starts from. It is not the parent and
  neither implies the other.

What a parent is for, in the words that sized it: "all could be projects, and
only the store really maps to subissues. the rest are too big to be
subissues" (Jack Heart, `scratch/said.md`). A parent with children is the
store-sized group: follow-ups of one Task, conducted from its conversation.
A group the size of the perf or Standard work is a Project, and its
conversation is a Wave's; that conversation conducting its Project's Tasks is
the same relation with a second source, and is not built here.

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
review and the conversation brings it up. Steps passing in between wake
nobody. `lf task complete LOO-937` closes the children with nothing unmerged
and names the one that still has an open PR, which stays open. After LOO-937 closes and its checkout is gone, `lf resume` reopens the
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
calling Task itself has an open parent, the new Task takes that same parent
and is its sibling. When that parent is done, abandoned or not on this
Machine, the new Task goes under the calling Task, so a follow-up is never
filed under work whose conversation nobody has open (slice 6).
`--parent <ID>` says otherwise; `--parent none` files top-level.
The caller is any Session that belongs to the Task, a step of its Graph
included, so Tasks filed by `launch-plan` or `ship-decomposed` are children
too. Outside a Task nothing changes. The link is written at filing and
nothing in this change rewrites it; a tracker edit to an issue's parent
arrives through the plan exchange (decision 5). A cycle is refused the way
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
parent moves. This sets aside the brief's "belongs by climbing". The later direction
supports it: with no Project nesting in Linear, "we might invert the design
somewhat and basically allow hierarchy to be grouping ontop of projecst"
(`scratch/said.md`), which keeps the Project as the fact a Task carries. A
parent later moved to another Project leaves its children where they were
filed.

**5. The parent travels as a plan field and as the tracker's own parent.**
`WorkValue::TaskParent` beside `TaskProject`. Linear: `parentId` on
`issueCreate` and `issueUpdate`, `parent { id }` in `issue_fields!`
(confirmed in Linear's published schema). Asana: `parent` on `POST /tasks`
and `opt_fields` (from memory of the API; not called). No Loopflow-only field
lands on a tracker item. This is the part that overlaps LOO-938 (PR #1983,
open; #1971 was closed): `planning_write.rs`, `work/edit.rs`,
`tracker_actions.rs`, and its new `plan_carriers.rs:125` `needs()`, which
must carry a parent before its child. It is built last, on LOO-938 via
`lf sync LOO-938` if that has not merged. LOO-938's plan says "Not built:
skipping a field this build does not know", so a peer on an older build may
fail on `task_parent`; with one person on current builds that is accepted.

**6. A parent's conversation is told when a child's standing moves.** No new
channel and no new writer: the same reading that writes a Task's updates
(`task_updates::write_task`) writes a second, coarser notice for whoever
conducts it.

- `session_tasks()` does not change. Widening it, as the kickoff draft had
  it, would deliver direction steered at a child to its parent's conversation
  (`steers.rs:411`, `p.task_id IN member_tasks`), make `conversation_task`
  return nothing for a parent (`moves.rs:165` matches exactly one Task), drop
  the Task from the context block and the steer marker
  (`context_block.rs:143`, `agent_process.rs:176`), and count a parent's
  terminal as hosting its children at retirement (`task_retirement.rs:182`).
- Conducted Tasks are the Tasks whose `parent_task_id` is one of the
  Session's Tasks. `task_updates::write` reads those after its own, and the
  steer audience (`scopes()`) admits a notice to a Session when the notice's
  Task has a parent among `member_tasks`. Both read one definition in
  `task_work.rs`.
- The notice is its own steer source beside `update:` and is written only
  when the child's standing moved: it stopped at an interactive step, a run
  ended failed or stopped, its PR merged, it was closed or abandoned. Steps
  passing inside a run are not news to a conductor, and neither is the same
  failure at the same step again: a relaunch that fails where the last run
  did says nothing until the Task's step or PR moves (slice 6). Each notice wakes an idle
  conversation into a paid turn, and today's `update:` steer fires at every
  step start; five children on an eight-step Graph would be forty turns.
- `TaskObservation` (`task_updates.rs:56`) gains the PR phase and whether the
  Task closed or was abandoned, so "landed" is news for the Task's own
  conversation too.
- The notice names the child, its step and `lf task status <ID>`, and reads
  as news about work this conversation conducts; today's line says "not a
  request or permission to launch work", which is wrong for a parent.
- Hearing follows the link and nothing else: a closed parent's conversation,
  once reopened, still hears about a child that is open.
- Delivery is unchanged: an idle Claude or Codex conversation is woken by the
  server, a busy one hears at its next tool or prompt hook, a closed one at
  its next resume. A missed wake is already logged and the news waits
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
- `lf task complete <parent>` never stops and takes no new flag **(made for
  review)**. It closes the parent, and with it: each child whose PR merged
  becomes done; each child never started (no checkout, no PR, no run) is
  abandoned, which destroys nothing. A child with anything unmerged (an open
  PR, a live run, a checkout with commits) stays open with its link, and the
  command names each one with its step. A child that becomes done this way
  settles its own children by the same rule, so completing a Task means one
  thing whichever command reached it (slice 6). Jack Heart: "3: i think probably
  close by default"; and on Asana, "There's some combination of completed and
  discarded".
- A child whose parent is done, abandoned or unknown to this Machine is
  listed top-level; its link is kept. One display rule covers all three.
- Passed over: `--keep <ID>` / `--abandon <ID>` and stopping the command
  (the kickoff draft), which add a refusal, two flags and a write that moves
  a child to another parent, all for a case `lf task abandon <ID>` already
  answers. Also passed over: abandoning unmerged children without being
  told, since abandonment force-removes the worktree and deletes the remote
  branch with no grace period; and marking never-started children done,
  which puts work nobody did in Done here and in the tracker.

**8. `--parent` reads the caller; `lf task create` help says so.** The line
"Plain creation files new work, regardless of the current checkout or
invoking Session" (`lf/mod.rs:2244`) and the adoption comment go.

## Docs, written first

The user docs for this change, describing intended behaviour; implement reads
them as the spec for wording and output:

- `docs/work.md`: "File follow-ups from a Task" (new), "Finish" (merged parent
  with open children, `lf task complete` over children, resume after close),
  the tracker tables, and Reference (Parent, Membership, Completion).
- `docs/sessions.md`: the `lf resume` table row for a directory that is gone;
  hearing about follow-ups.
- `docs/agent-api.md`: Task updates and the notice to a conductor.
- `docs/glossary.md`: Parent, child.

Settled while writing, for review: the human `lf task status` prints one
`child:` line per child (identifier, title, standing) and the reason line;
`lf task complete <parent>` prints one line per child with what happened to
it; the notice reads "LOO-15 "…", which this conversation conducts, stopped
at human-review. lf task status LOO-15".

## Source of truth

`tasks.parent_task_id`. Children are the Tasks that name a parent, ordered by
filing time; no stored child list. Open, merged and closed derive as today
(`task_state_sql`, `sqlite/durable.rs:346`).

## Surfaces

| Surface | Change |
|---|---|
| `lf task create` | `--parent <ID\|none>`; default from the calling conversation; receipt carries `parent` |
| `lf task status --json` (`task_row::TaskDetail`) | `parent`, the parent's `TaskRow` or `null`, beside `stack_parent`; `children: [TaskRow]`, the row type every list already uses, not a second shape |
| `lf task show` (`TaskStatus`, `DesktopTaskPane`) | same two keys; human output lists children |
| `TaskRow`, `RoadmapTask` (lists, server, Swift mirror) | `parent_id`, the parent's stable ID, required and nullable, so a list can nest and the sidebar has something to draw |
| `lf wave show` | a child prints indented under its parent |
| `lf task complete` | closes merged and never-started children; names the ones left open; `--json` and served `task.complete` carry `children`, one entry per child with what happened to it |
| served `task.create` | `parent` |
| Task updates | merge and close for a Task's own conversation; a notice to its parent's when its standing moves |
| Fixtures | `task_detail.json`, `task_status.json`, `roadmap_snapshot.json`, `task_create_receipt.json`, `local_task.json`, `server_schema.json`, Swift `DTOFixtureTests` |

Absent and error states: no parent is `null` and no children is `[]`, never a
missing key. A parent that is done, abandoned or unknown to this Machine leaves
the child top-level with its link kept. `--parent` naming an unknown Task is
an error naming it. A conversation with no Task files top-level, as today.

## Operational limits

- The conducted-Tasks lookup runs on every hook boundary of every
  interactive conversation. It is one indexed read on
  `tasks(parent_task_id)`; check `EXPLAIN QUERY PLAN` against a 10,000-Task
  store.
- A hook starts without the quiet cache (`QUIET_AT` is per process), so it
  rebuilds each Task's history, "tens of milliseconds" each
  (`task_updates.rs:24`), inside a ten-second hook. The loop over children
  skips a child whose last reading already reported it closed or abandoned.
  Measure a boundary with ten open children; if it is slow, the reading row
  keeps the revision it was taken at so an unmoved child costs one compare.
- Notices for children are written inside the reading Session's hook or the
  server's watcher, as today; no new writer, no new lock holder.

## Exclusions

- The sidebar (the brief's fourth slice, not slice 4 below): played with in
  the running app once children exist.
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

1. **A closed Task's conversation resumes.** Built (17d81aa538).
   `admit_workspace` is the one place: a Session whose `cwd` is not a
   directory takes its repository's main checkout (from its Task, else its
   `repo`, else its Wave) and the row keeps the new `cwd`. Shown by
   `session_cli_tests::a_closed_tasks_conversation_resumes_in_the_main_checkout`
   (`lf -b session resume <id> --message …`, Claude stand-in, `pwd` is the
   main checkout) and `retirement::tests::a_closed_tasks_conversations_reopen_in_the_main_checkout`
   (real retirement, primary and bound Sessions, `lf resume <Task>`).
   What it taught: the plan held, and the launch path had three more places
   that demanded the checkout, none of them in the plan: binding a Task as
   Work (`ops/run.rs` `bind_selected_work`, now `task_directory`), the launch
   prompt's workspace context (`task_workspace_context`), and two
   `canonicalize` comparisons that failed on a missing path
   (`agent/session.rs`, `commands/run.rs`). An open Task with no checkout
   still refuses as before; only a done or abandoned one falls back.
   Not covered: a Codex resume (the launch path is shared; only the Claude
   stand-in ran) and the interactive viewer path, which reaches the same
   `admit_workspace` through `open_waiting`.
2. **The link, local.** Partly built (62e08279d5).
   Built: migration `task_parent` (`parent_task_id`, `tasks_parent` index,
   the planning trigger's `WHEN`), `Task.parent_task_id`,
   `lf task create --parent <ID|none>` and the default from
   `journal::caller_session_id()`, flat by default, Project and Wave copied
   when the parent is in the same repository, `task_children`, `parent` and
   `children` on `lf task status --json`, `parent:` and `child:` lines in
   the human output. Shown by
   `session_cli_tests::a_task_filed_from_a_tasks_conversation_shows_under_it`
   and `adoption::tests::a_task_filed_from_a_tasks_conversation_is_its_child`.
   Built with slice 4: `parent_id` on `TaskRow` and `TaskSummary` (the
   parent's stable ID, so a list nests by `id`; `TaskRow`'s key was `parent`
   until slice 6),
   the Swift mirror `TaskPlanningSnapshot.parentId` and every fixture,
   `lf wave show` and `lf task list` indenting a child under an open
   parent in the list.
   Built after slice 4: `lf task create --json` carries `parent`, the
   parent's `TaskRow` or `null` (`created_rows` reads both rows in one
   snapshot); served `task.create` takes `parent`, required and nullable,
   and Swift's `ServedWrite.taskCreate` sends it (every caller passes
   `nil`); `lf task show --json` (`DesktopTaskPane`) carries `parent` and
   `children` as `lf task status` does; `task_prs.after_merge` /
   `next_slug` and the `pr_landings` pair are out of every read and marked
   `-- leaving:` LOO-996. Shown by
   `session_cli_tests::a_task_filed_from_a_tasks_conversation_shows_under_it`
   (`.parent.identifier` on the receipt, both keys on `lf task show`) and
   the `dto_fixtures::` round trips of `task_create_receipt.json`,
   `task_status.json` (now read as `DesktopTaskPane`) and
   `subscribe_frames.json`. Run on 3514139603, 2026-11-18: all 50
   `dto_fixtures::` tests and both parent `session_cli_tests` passed.
   Not built: Swift does not decode the receipt's `parent` or the pane's
   two keys (it has no `TaskRow` mirror and nothing draws them yet); the
   iOS target was not compiled here, only the macOS Swift tests.
   The cycle refusal was built with slice 5.
3. **Hearing about children.** Built. `task_work::conducted_tasks` is the
   one definition; `task_updates::write` reads the Session's conducted Tasks
   after its own (`session_conducted_tasks`: id and closed, two columns, no
   Task row), and `scopes()` admits `notice:<Task>:<key>` steers to the
   conductor only. `TaskObservation` has `pr` and `closed`; the Task's own
   update says "PR merged." and "Task completed.". Triggers
   `task_update_tasks_standing` and `task_update_task_prs_merge` are in
   `task_parent.sql`. `task_session.md` has the conducting paragraph.
   Shown by `harness::task_updates::tests::` (a child stops at an
   interactive step: one notice, none for its steps, none to the child's
   own conversation, a steer for the child stays with it; failed run,
   merge, close; a child nobody read) and
   `a_hook_reading_its_steers_and_task_updates_plans_by_index`, which
   traces the new lookup and the wider audience and finds no scan.
   What it taught: a first reading as a baseline loses a child that stopped
   before anyone looked, so a notice compares a missing reading with an
   empty one; and `store.task()` cannot read the sparse `retirement`
   fixture row, which now carries a title, slug and times.
   Not built: the `MachineServer` fixture (a real child Graph, the server
   waking the parent). Delivery is the unchanged steer path; the acceptance
   check at gate is where it runs end to end. Not measured: a hook boundary
   with ten open children.
4. **Merged and open; closing.** Built. A merged Task with open children
   reads "Merged · N children open" with `next` null (`task_actions::
   merged_action`, `snapshot_task_detail`, `task_row::next_command`). A
   run reaching `end`, and Auto-Ship's merge, request no completion while
   children are open (`request_completion`); `lf ship` prints the reason
   and returns (`ShipMerge::Conducting`). `lf task complete` records a
   `family_completion_requested` event, so a completion that waits for the
   parent's merge still settles the children when it lands; one
   transaction closes the parent, completes merged children and abandons
   never-started ones (`complete_children`), and the command prints one
   line per child: done, abandoned, left open. A child whose parent is
   closed or not in the list prints top-level (`task_hierarchy`).
   Shown by `session_cli_tests::a_merged_parent_stays_open_until_it_is_completed_with_its_children`
   (the CLI end to end over seeded PRs: acceptance steps 3 and 7; passed
   alone on e40719cb37, 2026-11-18),
   `task_work_lifecycle_tests::reaching_end_leaves_a_task_with_open_children_open`
   and `task_actions::tests::a_merged_task_with_open_children_is_not_asked_to_complete`.
   What it taught: a parent that reaches `end` with children open is not
   approved to ship either, so a PR still unmerged there waits for
   `lf land`; and "never started" needed its own definition (no PR, no
   Graph process, no checkout event, not begun), stricter than
   `unstarted`.
   Not built: Desktop does not nest yet (`parentId` is decoded and
   unused). A done parent run through `lf task complete` again does not
   revisit children left open.
   The three review items, settled:
   - "Merged with children open" is `task_actions::conducting_reason`,
     read by `merged_action` and the list's next move; `RoadmapTask`
     carries its answer (`conducting`) for `TaskRow` and `lf wave show`.
   - `TaskEventKind::FamilyCompletionRequested` stays. The other form, a
     Graph's request withdrawn when a child appears, needs every write
     that can put an open child under a waiting Task (filing, reopening,
     slice 5's parent edit) to remember to withdraw it; one missed and a
     Graph's request abandons a child nobody asked to close. The variant
     says why in its doc comment.
   - `durable::abandon_task_in` is the one abandon write; `lf task
     abandon` and `complete_children` both call it.
5. **The link travels.** Built, on LOO-938 (`lf sync LOO-938`, merge
   50ee8cb3c8). `WorkValue::TaskParent` (`task_parent`) is a plan field:
   creation writes it through `planning_write::create` (the raw `UPDATE` in
   `insert_task_row` is gone), a carrier's page holds a parent before its
   child (`plan_carriers::needs`, same repository only), and the queue's
   `create` carries it. Linear: `parentId` on `issueCreate` and
   `issueUpdate`, `parent { id }` in `issue_fields!` and
   `LIST_ITEMS_QUERY`. Asana: `parent` on `POST /tasks`, the parent's gid
   read with one `GET /tasks/external:<id>`. A child's creation waits in the
   drain while its parent's is unsent (`parents_named`, one set for Projects
   and Tasks). `tracker_items.parent_id` and `parent_id_observed`
   (`task_parent.sql`) hold what the tracker says; `observe_parents` applies
   it after a batch is admitted, so a sub-issue read before its parent still
   links. The cycle refusal is `planning_write::parent_refusal`: a local
   save errors, a tracker's or another Machine's value is skipped and the
   Task keeps its parent.
   Shown by `plan_carriers::tests::a_follow_up_reaches_another_machine_under_its_parent`,
   `linear_events::tests::a_trackers_parent_becomes_the_tasks_and_never_makes_a_task_its_own_ancestor`,
   `task_planning_tests::a_follow_up_is_created_in_linear_under_its_parents_issue`
   and `asana_planning_tests::asana_follow_up_is_created_as_a_subtask_of_its_parent`.
   Run 2026-11-18: those four; all 373 `loopflow-storage` library tests;
   execution's `ops::linear::`, `linear::`, `ops::tracker`, `asana::` and
   `ops::plan_` (148 of 151); both parent `session_cli_tests`; clippy.
   What it taught: a tracker with no parent must not clear a link it could
   never hold (a parent in another repository, or one never sent), so
   "no parent" clears only when the held parent has a tracker item; and a
   parent in another repository cannot ride a repository's page, or the
   other Machine files it in the wrong repository.
   Not built:
   - Asana reads no parent. Its API names a parent by gid and Loopflow
     keeps only its own `external` id, so `TrackerItem.parent_observed` is
     false for Asana and a subtask made in Asana does not become a child.
     `docs/work.md` now claims the Linear half only; whether Asana gids are
     stored so it can be read is open (`scratch/questions.md`).
   - Asana's edit of a parent retires as unsupported. Nothing local changes
     a parent after filing, so no `task_parent` field action is queued by
     either tracker today; Linear's `issueUpdate` arm has no caller yet.
   - A child whose parent's `create` was rejected for good is sent with no
     parent once the parent has no queued creation, and nothing sends the
     link later.
   - A link skipped on import still takes the arriving stamp, so the two
     Machines keep different parents until one is edited again.
   - `test(local_planning::)`, `binary(two_people)` and the Swift suites
     were not run here: gate.
   Found at realign and fixed: slice 5's fixture edit had also put
   `"parent":null` into `linear/actions.rs` `move_input`, the `issueUpdate`
   input a reorder sends. `IssueUpdateInput` has `parentId` and no `parent`
   (Linear's published schema, as decision 5 read it), so a real reorder
   would have been rejected. The fake Linear in `planning_order_tests`
   accepts any key: all four passed before and after, so no test here
   notices an unknown key in a mutation input.
   Failing on this merge and not from this slice:
   `planning_lookup_tests::missing_git_task_never_falls_through_to_linear`
   (expects "not found in the Git tracker", a message LOO-938 removed) and
   `ops::linear::tests::connect_adds_an_unregistered_wave_on_main_before_linking_it`
   ("missing field `teams`"). `plan_git::tests::a_task_made_on_one_clone_…`
   failed twice from this checkout and passed from an empty directory with
   a temp `HOME`.

6. **What the built branch showed.** Built. The concept review of the
   whole branch (`scratch/concept-review.html`, 2026-11-18, 3e7bbedb16)
   found five places where one idea has two behaviours or two spellings.
   Choices are **made for review** (`scratch/questions.md`).
   - **Flatten only to an open parent.** `TaskParentChoice::resolve` takes
     the calling Task's parent only when that Task is on this Machine and
     neither done nor abandoned; otherwise the calling Task. The "unknown
     Task" error is now only reachable with `--parent`. Shown at the end of
     `adoption::tests::a_task_filed_from_a_tasks_conversation_is_its_child`.
   - **Completing a Task settles its children, by any path.**
     `complete_children` recurses into a child it completes, inside the one
     transaction; `ChildCompletion.children` carries the result, the text
     output indents it, and tracker sync and retirement follow every level.
   - **The same failure is said once.** `TaskObservation::news` drops a
     failure sentence a completed run in the old reading gave, while the
     Task's step (`TaskGraphPosition`'s `step`) and PR phase are what that
     reading had. Shown by
     `task_updates::tests::a_relaunch_failing_where_the_last_run_did_is_not_told_twice`.
     Not covered: the step or PR moving and the same failure being told
     again; the fixture has no Graph position.
   - **One spelling for the parent on the wire.** `TaskRow.parent_id` is the
     ID; `parent` is the parent's `TaskRow` or `null` on `lf task create`,
     `lf task status` and `lf task show`. `roadmap_snapshot.json` and
     `subscribe_frames.json` needed no edit: the first already said
     `parent_id`, the second holds a `null` parent.
   - **An agent can read what its completion did.** `lf task complete
     --json` and served `task.complete` answer `TaskCompleted`: the Task's
     fields as before plus `children`, each `{task, outcome, children}` with
     `task` the child's `TaskRow` read after the settlement.
     `ChildDisposition` became `ChildOutcome` (`done`, `abandoned`, `open`)
     and is the wire value.
   Both CLI cases are in
   `session_cli_tests::a_merged_parent_stays_open_until_it_is_completed_with_its_children`
   and `a_task_filed_from_a_tasks_conversation_shows_under_it`.
   Run on 8100b36272, 2026-11-18: all 13 `harness::task_updates::tests::`,
   the `adoption` test, both `session_cli_tests` and all 48
   `dto_fixtures::` passed.
   Read at realign: `TaskObservation::step` is the step the Task stands at
   or its run left (`TaskGraphPosition::Run.step`, not `at`), so a relaunch
   read mid-run does not reset the silence; only the Task arriving at
   another step or its PR phase moving does.
   Left for gate: `binary(served_reads)` (it compares served `task.complete`
   with the CLI's `--json`; both go through `task_completed`),
   `test(documented_commands::)`, and Swift, which decodes `task.complete`
   as a Task and none of `parent`, `parent_id` or `children`.
   Passed over, left as they are: notices to the conversation that just
   completed the parent (one prompt, already carrying its own update);
   `lf task create` typed in a terminal in the Task's checkout filing
   top-level (the caller is the conversation; making the checkout count is
   new behaviour); `TaskEventKind::FamilyCompletionRequested`, a record the
   person never sees, kept for the reason in its doc comment.

Next: gate. QA walked the acceptance check by hand
on a private Machine, 2026-11-18, before slice 6: every line of it held,
and a real Claude conversation was woken two seconds after its child stopped.
A second pass on 26683764f9 (`scratch/qa.html`) walked it again with slice 6
and changed no code: the flat rule after a parent closes, the nested
settlement, `children` on `lf task complete --json` and served
`task.complete`, served `task.create {parent}`, the link on a second Machine
through the Git origin, a Graph reaching `end` on a parent with a child open,
and six failures of one step waking real Claude once, with the same failure
news again after the child had stood at `demo` (the case slice 6's test does
not cover). What it left open is in `scratch/questions.md`.
A prompt hook for a parent with ten children at a review step took about
600 ms against 510 ms for a childless Task, both on a debug build. Left for
gate: the `MachineServer` fixture test of the same check and the suites named
above. Swift still does not decode the receipt's `parent` or the pane's two
keys.

One PR. #1983 (LOO-938) was open and unmerged at 2026-11-18; this branch is
stacked on it and its PR waits behind #1983. The two LOO-938 test failures
above come with it.

## Acceptance check at gate

A fixture test, headless, with a `MachineServer`:

```bash
lf task create 'follow-up' --json             # from LOO-A's conversation: .parent.identifier == "LOO-A"
lf task status LOO-A --json                   # .children[0].identifier is the follow-up, its .parent_id LOO-A's .task.id
# LOO-A's PR merges
lf task status LOO-A --json                   # .task.pr.phase "merged", .task.state not "done", .task.next null
# the follow-up's Graph stops at an interactive step
lf session steers <LOO-A's conversation>      # an update naming the follow-up and its step
lf steer <follow-up> 'x'; lf session steers <LOO-A's conversation>   # 'x' is not there
lf task complete LOO-A                        # closes LOO-A; the merged follow-up is done, an unstarted one abandoned, one with an open PR named and left open
lf task retire && lf -b resume <LOO-A's conversation> --message 'pwd'   # answers from the main checkout
```

## Done when

From a Task's conversation, a follow-up is filed with no flag and shows under
that Task; the conversation is told when the follow-up stops, fails or lands;
the Task's PR can merge and the Task stay open without being told to
complete; closing it closes the children that have nothing unmerged and
names the ones it leaves open; and
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
