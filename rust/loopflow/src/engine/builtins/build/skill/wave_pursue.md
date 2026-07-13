---
description: Advance the wave in one turn — keep it computable, pursue a bet, evolve it.
default_agent: codex
action_style: procedural
---
Pursue the wave objective.

One turn does whatever the wave most needs now: clarify the artifact, pursue a
bet, or evolve from what landed. Do the few that have drifted — not all three
every pass. The controller decides whether the wave runs again, waits, or is
nudged; you never write a loop bit.

## Orientation

Resolve the exact wave from the prompt or its `wave/<wave>/GOAL.md`; never infer
an approximate name. Read GOAL/MEMORY, the PM snapshot's Project definitions,
KRs, and tasks, recent chat, and Task Session state. If that reader fails,
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

## Pursue

- Select from filed tasks and open KRs; filing work does not require launching it.
- Keep coordination and small read-only decisions in the Wave. Every concrete
  file-writing change begins as a Linear task under exactly one Project.
- Start the task with `lf task run <issue-id> --directive "<delegation brief>"`.
  The resulting Task Session owns one immutable worktree, provider transcript,
  and pull request to main.
- Supervise active work with `lf task status`, `lf task steer`, `lf task
  interrupt`, `lf task wait`, and `lf task resume`. A second task may run in
  parallel when capacity permits; never create a second session for one issue.
- A linked `decision_requested` event is a question from the Task, not human
  speech. Answer it once with `lf task decide <issue> <decision-id> <choice>
  [--message "feedback"]`. Inspect delayed command acceptance with `lf task
  receipt <command-id> --until applied --timeout 30s --json`; use `--until
  incorporated` when the semantic acknowledgement matters.
- Use `lf project run <linear-project-id> --directive "<delegation brief>"` to
  create or resume the Project's durable pursuit session. It sleeps while
  supervised Tasks run and wakes from their typed observations. Projects never
  own worktrees or Waves.
- Trust linked Task events and summaries. Drill into the Task Session only when
  the report is insufficient; do not copy raw child tool chatter into the Wave.
- Answer human steering before returning to the goal.

## Keep the wave true

Only touch these when they have drifted from what actually landed:

- Edit `wave/<wave>/GOAL.md` when the objective, measures, bounds, or cron
  intent no longer ask the honest question. Do not implement product work here
  beyond a trivial correction to the artifact itself.
- Update the authoritative Linear Project, then run `lf pm sync`, when a
  definition or KR set has drifted. KRs read as proof under duration: counted,
  unattended, endurance-shaped end states on real work — not backlog bullets,
  issue ids, status, or implementation receipts. If a "project" is really
  individual cleanup, file it as a task under a broader project.
- Reconcile KRs against reality: retire one only when its condition verifiably
  holds — a counted streak isn't satisfied by one good day; a human rescue
  inside an unattended window resets it. Write changed KRs with `lf pm project
  update`; archive dead bets with `lf pm project archive`.
- Add durable learnings with `lf memory add`, or curate memory through the
  server-owned memory command when the accumulated facts need it.
- Launch, retire, reset, or split sub-waves when the objective needs a new
  center of work. Escalate real blockers upward with `lf radio pub --parent`.

The wave never terminates; it changes shape. Stopping is not a runtime
decision. Keep the turn focused: do the one or two things the wave most needs,
then let the controller schedule the next pass.
