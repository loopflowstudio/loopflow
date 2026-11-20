# LOO-995 — a Task can be in a Wave with no Project

PR #1997 removed the chapter machinery and is held, unmerged. This slice
changes how a Task reaches its Wave.

Jack Heart, 2026-11-20: "i think a wave can optionally have a project
associated with it", "if it doesnt, then wehn you a tasks to it, it just goes
into the project of the parent", then, choosing among three answers for a
Wave with no Project anywhere above it: "(b) the Task joins the Wave directly
with no Project, which adds a wave_id on the Task". On trackers: "then the
question becomes just how we tell with a tracker that a task in a certain
wave".

## The rule

A Task's Project is optional. A new Task added to a Wave:

1. joins that Wave's Project, chosen as today when it has one;
2. else joins the nearest ancestor Wave's Project;
3. else has no Project.

In all three the Task records its own Wave (`tasks.wave_id`). Nothing
creates a Project for a Task any more: `join_project_in` making one named for
the Wave goes, with its callers (`children.rs`, `task_content.rs`,
`test_fixture.rs`).

A Task's Wave is `tasks.wave_id`, not read through its Project. Every reader
that reaches a Wave through `tasks.project_id → projects.wave_id` moves to
`tasks.wave_id`. A Task in an ancestor's Project still lists in its own Wave.

## Trackers

Linear and Asana hold an issue in a Project, never in a Wave. A Task in an
ancestor's Project is that Project's issue in the tracker; which child Wave
it is in lives only in Loopflow's plan, carried by Git and the hub like any
other field. A Task with no Project sends nothing to a tracker. A Task created
in the tracker joins the Wave linked to its Project. Marking the child Wave on
the issue (a description marker or an Attachment) is not built; LOO-980 owns
the tracker link.

## Schema

`tasks.wave_id` is a new column in `remove_chapters.sql`, filled for existing
rows from their Project's Wave. It is a stamped planning field: the plan
record, `plan_changes`, the DTOs and fixtures carry it. Check the Wave
listing's query with `EXPLAIN QUERY PLAN` at about 10,000 Tasks.

## Made for review

- A Wave may still hold several Projects; this slice does not narrow it to
  one. Asked, not yet answered.
- Without a tracker, nothing makes a Project once a Task no longer does, so
  a repository with no tracker has none. Existing Projects stay. Whether a
  command should make one is open.
- Moving a Task between Waves keeps its Project only when the new Wave is
  that Project's Wave or a descendant of it; otherwise the rule above
  places it again.

## Built (slice 1)

- `tasks.wave_id` (migration, index `tasks_wave`, trigger) and plan field
  `task_wave`; `create_task_in` takes the filing Wave and applies the rule
  (`task_project_in`, recursive over `parent_wave_id`). `join_project_in` is gone.
- The planning writer sets `task_wave` whenever a `task_project` lands without
  one and the Task is not already in that Project's Wave or below it. Tracker
  imports and records from builds without `task_wave` go through it.
- `move_task` writes both fields; the move preview uses `task_move_project`.
- Storage readers (listing, counts, steers, CI incidents, Session completion,
  `task_wave_in`) read `t.wave_id`.
- A Task in a Wave with no Project gets no tracker `create`; a standalone Task
  (no Wave) still goes to the tracker as before.
- Test: `local_planning_tests::a_task_joins_its_wave_project_else_its_nearest_ancestors…`;
  plan test `query_plans::a_wave_lists_its_own_tasks_by_index`.

Demo run with this build on an isolated home: `x` in `billing/refunds` has no
Project, lists under `billing/refunds` and not `billing`; the store has 0 Projects.
The Linear half of the demo is not run here.

Checks:
- `cargo test -p loopflow-storage --lib`: 365/367 passed, then both fixture
  failures fixed and rerun (steers, task_work). Run locally.
- `cargo test -p loopflow-execution --lib` for `store::membership_lifecycle_tests`,
  `store::ownership::tests`, `store::admission_lifecycle_tests`, `ops::task::explain::tests`:
  pass. Run locally.
- clippy on storage and execution: clean. Run locally.
- `documented_commands::`, `binary(golden_prompt)` (the `split-wave` builtin
  changed), Swift DTO fixtures and the full suites: for gate and CI.

Raw-SQL `INSERT INTO tasks` without `wave_id`: only
`rust/loopflow/tests/build_cleanup.rs:487` copies a `project_id`, and it reads
no Wave; the rest set no Project. Nothing left to implement in this slice.

QA (2026-11-20, branch `lf` on isolated homes; `scratch/qa.html`): first
visit, a home from the installed base `lf` opened by the branch, a second
Machine over Git, moves, and a Linear queue with no login. Fixed: a Task in
an ancestor's Project showed as "unavailable" under its own Wave; a first Git
import left a Task without a Wave when its Project came in the same batch;
`lf task create` printed "pending Linear sync" and `lf task sync` failed for
a Task that never goes to the tracker (`Task::goes_to_tracker`); five
`local_planning` tests still expected the old rule.

Next: gate publishes the branch so CI runs the suites on these commits (PR
#1997 shows no run since they landed).

## Shows it worked

```bash
lf wave create billing ; lf wave create billing/refunds
lf task create --wave billing/refunds 'x'      # no Project; lf wave status billing/refunds lists it
# with Linear: link billing to a Linear Project (lf wave plan connect billing)
lf task create --wave billing/refunds 'y'      # in billing's Project; still listed under billing/refunds
```

Docs: `docs/work.md` "Projects" ("Its first Task makes one" goes).
