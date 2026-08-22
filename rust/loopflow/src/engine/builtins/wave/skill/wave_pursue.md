---
description: Direct the Projects and Tasks that advance the Wave.
action_style: procedural
---
Pursue the Wave objective from the clarification produced earlier in this
flow.

## Orientation

Resolve the exact wave from the prompt or its `wave/<wave>/GOAL.md`; never infer
an approximate name. Read GOAL/MEMORY, the PM snapshot's Project definitions,
KRs and tasks, the selected Turn, Task state, and the seeded
`metric_portfolio`. If that reader fails,
report the failure once and select from memory; repairing PM or auth is not
this wave's new objective. Trust worker summaries; do not reread worker
transcripts. Each project belongs to exactly one wave and owns KRs, not memory
or cadence.

Planning commands belong at this tier:

```bash
lf pm show --wave <exact-wave>
lf pm task create --project <project> --title "..." --notes "..."
lf pm task update --id <task-id> --title "..."
```

## Task references

When selecting, supervising, or reporting more than one Task, read `lf roadmap
--wave <exact-wave> --json` for plan-wide rows and `lf status <exact-wave>
--json` for live execution. Render every Task with the shared reference:

```markdown
[identifier](provider URL) — readable active PR/workspace slug — status/next owner
```

Fill the link from `task.identifier` and `reference.issue_url`. Use
`active_pr.slug` from roadmap; for status, match `active_pr` to `prs[].id` and
use that PR's `slug`. Fall back to `reference.workspace.slug`. Take status from
`runtime.status`, or from the roadmap `section` when runtime is absent;
`next_move.owner` supplies next owner. Never reconstruct a provider URL, branch,
or slug from an identifier, title, worktree, or naming convention. Omit only a
link or slug whose snapshot evidence is explicitly absent; keep the Task and
its available status/next owner.

## Pursue

- Select from filed tasks and open KRs; filing work does not require launching it.
- Route missing, stale, or unavailable instrumentation to the metric's owning
  Project. A different Project may use that reading for KR proof, but name the
  true owner and pass the cross-owned evidence explicitly in the Project
  direction.
- When an important Project outcome is still invisible, direct its Project to
  consider sponsorship; do not define the metric at Wave level. Pass relevant
  cross-Project evidence explicitly.
- When an active Project has actionable KR evidence but no outcome worker,
  direct the Project to choose its next experiment. Let it yield when evidence
  needs time, a dependency blocks progress, or no justified move remains.
- Treat Projects as the Wave's bet portfolio. When one Project has an unresolved
  mechanism choice, direct its Project to run an approach portfolio;
  do not create duplicate Projects or several Tasks with the same favored
  brief. Allocate attention from concrete Task evidence and exact gaps, not
  activity or symmetrical fan-out. Let the Project preserve early independence
  and synthesize the routes; the Wave judges whether the bet still earns
  attention against its siblings.
- Keep coordination and small read-only decisions in the Wave. Every concrete
  file-writing change begins as a Linear task under exactly one Project.
- Start the task with `lf task run <issue-id> --directive "<delegation brief>"`.
  This ensures the owning Project before creating the Task.
  The Task owns one stable worktree and provider transcript. Its ordered
  PRs own serial branches to main; the Project receives routine
  observations and decisions.
- If a separate Task depends on an open parent PR and should start now, run
  `lf task run <child> --stack-on <parent> --directive "..."`. It gets a
  separate worktree and worker; same-Task PRs remain serial.
- Supervise active work with `lf task status`, `lf task steer`, `lf task
  interrupt`, `lf task wait`, and `lf task resume` when root inspection or
  override is needed. This never replaces the Task's Project.
  Independent tasks may run in parallel; never create a second session for one
  issue.
- Use `lf project run <linear-project-id> --directive "<delegation brief>"` to
  create or resume the Project's durable pursuit session. It sleeps while
  supervised Tasks run and wakes from their typed observations. Projects never
  own worktrees or Waves.
- Trust linked Task events and summaries. Drill into the Task only when
  the report is insufficient; do not copy raw child tool chatter into the Wave.
- Answer human steering before returning to the goal.

Keep the turn focused on the one or two useful actions selected by the clarify
phase. The mutate phase judges the resulting evidence; write no loop bit.
