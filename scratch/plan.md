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

## Shows it worked

```bash
lf wave create billing ; lf wave create billing/refunds
lf task create --wave billing/refunds 'x'      # no Project; lf wave status billing/refunds lists it
# with Linear: link billing to a Linear Project (lf wave plan connect billing)
lf task create --wave billing/refunds 'y'      # in billing's Project; still listed under billing/refunds
```

Docs: `docs/work.md` "Projects" ("Its first Task makes one" goes).
