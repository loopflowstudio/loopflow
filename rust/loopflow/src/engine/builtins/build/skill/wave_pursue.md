---
description: Advance one wave bet inline first, with bounded delegation.
default_agent: codex
action_style: procedural
---
Pursue the wave objective.

## Orientation

Resolve the exact wave from the prompt or its `wave/<wave>/GOAL.md`; never infer
an approximate name. Read GOAL/MEMORY, the local PM snapshot, recent chat, and
worker state. If the PM reader fails, report the failure once and select from
memory;
repairing PM or auth is not this wave's new objective. Trust worker summaries;
do not reread worker transcripts.

Planning commands belong at this tier:

```bash
lf pm show --wave <exact-wave> --no-sync
lf pm task create --project <project> --title "..." --notes "..."
lf pm task update --id <task-id> --title "..."
```

## Work

- Select from filed tasks and open KRs; filing work does not require launching it.
- Keep coordination and small read-only decisions in the Wave. Every concrete
  file-writing change begins as a Linear task under exactly one Project.
- Start the task with `lf task run <issue-id>`. The resulting Task Session owns
  one immutable worktree, provider transcript, and pull request to main.
- Supervise active work with `lf task status`, `lf task send`, `lf task
  interrupt`, `lf task wait`, and `lf task resume`. A second task may run in
  parallel when capacity permits; never create a second session for one issue.
- Use `lf project run <linear-project-id>` to queue a Project directive for its
  owning Wave. Projects never own worktrees or child minds.
- Trust linked Task events and summaries. Drill into the Task Session only when
  the report is insufficient; do not copy raw child tool chatter into the Wave.
- Answer human steering before returning to the goal.

Keep the turn focused: select, create, supervise, or respond to results.
