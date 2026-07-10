---
description: Advance one wave bet inline first, with bounded delegation.
default_agent: codex
action_style: procedural
---
Pursue the wave objective.

## Orientation

Resolve the exact wave from the prompt or its `wave/<wave>/GOAL.md`; never infer
an approximate name. Read GOAL/MEMORY, project docs, recent chat, and worker
state. Read live Linear tasks when the exact wave has PM configured. If that
reader fails, report the failure once and select from project KRs and memory;
repairing PM or auth is not this wave's new objective. Trust worker summaries;
do not reread worker transcripts.

Planning commands belong at this tier:

```bash
lf pm show --wave <exact-wave>
lf pm task create --project <project> --title "..." --notes "..."
lf pm task update --id <task-id> --title "..."
```

## Work

- Select from filed tasks and open KRs; filing work does not require launching it.
- Execute the next move inline by default. If one concrete blocker stands
  between the wave and progress, resolve it in this process instead of creating
  another worktree, vendor session, task, or loop.
- Create a project or task loop only when the child is a strict subset of the
  wave objective and needs an independent multi-pass lifecycle, its own PR, or
  useful parallel execution. Never delegate the whole wave objective.
- Inhabit such work with `lf --wave <exact-wave> loop <project-or-task-flow>
  "<whole handoff>"`. A seed that cannot finish without delegating the parent
  objective again is not a handoff.
- `--detach` changes ownership of an already-justified loop; it is not a reason
  to create one. Add it only when the wave has another useful move while the
  child runs. If the result gates the next move, keep the loop foreground.
  Detaching requires an already-served exact wave; do not start a server merely
  to make it available.
- Require detached hands to report with `lf radio`, record live learnings with
  `lf memory add`, and leave done as a PR. No writes means failed work.
- Watch a hand with `lf sub <channel>`. You already hear its reports: they land
  in this thread, attributed, even the ones broadcast while you were asleep. To
  change what a hand does, say it here — hands re-read this thread at every pass
  boundary; a `lf radio --channel` broadcast only reaches whoever is tuned in
  right now.
- Answer human steering before returning to the goal.

Keep the turn focused: select, execute, or delegate a strict subset; record what
changed.
