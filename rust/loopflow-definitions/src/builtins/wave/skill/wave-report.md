---
requires: .lf/wave/
produces: scratch/wave-report.md
---
Read health signals across all Waves and show where the system is stressed.

The report is a triage, not a status update: someone opens it and knows where
to spend their next hour. Which Waves need thinking, which can run on their
own, and which are failing quietly.

Read `lf wave list --json`, then `lf wave status <wave> --json` and
`lf wave show <wave> --json` for each, with its `GOAL.md` and `MEMORY.md`.
Gather git and PR history for each Wave's area. Collect all five signals
before interpreting any of them.

## Signals

**1. Staleness.** Time since the Wave last moved: merged PRs, Tasks completed,
commits in its area over two weeks. Silence in a Wave with nothing started is
healthy; one line and move on. Silence with started Tasks or open PRs is a
stall: how long, and what happened last? Activity that suddenly stops is
sharper than chronic quiet.

**2. Dependency pressure.** Which Waves are load-bearing? Count the Tasks in
other Waves that wait on this one; six Tasks in three Waves waiting on one is
real pressure. Overlapping areas with no stated dependency are uncoordinated
coupling, which is merge-conflict territory. Propose naming a dependency you
find undeclared.

**3. Design debt.** Signs the Wave pushed forward and got pushed back: PRs
closed unmerged, branches ahead of main with no PR, briefs rewritten again
and again without shipping. One abandoned PR is iteration. Three on the same
Wave means the direction is unclear, and it needs design thinking, not more
build cycles.

**4. Velocity mismatch.** Activity against backlog. Many commits and a
shrinking backlog: converging. Many commits and a flat or growing backlog:
thrashing, with Tasks too big or badly scoped. Few commits and a flat backlog:
dormant, fine if intended. PRs that land in minutes may be shallow.

**5. Coherence.** Do the Wave's Tasks and KRs pull toward its objective, or is
it a dumping ground? A Task that belongs to another Wave's concern, Tasks
spanning four subsystems, briefs older than the last major redesign.

## Report

Score each Wave on each signal as **quiet**, **warm** or **hot**. Three levels
are enough; no weighted formulas. Write `scratch/wave-report.md`, hottest
first:

```markdown
# Wave Report — <date>

## Attention needed
<Hot Waves: which signals, the specifics, and what kind of attention —
design thinking, unblocking, descoping, splitting, or landing existing work>

## Warming
<What to watch.>

## Quiet
<One line each.>

## Signal map
| Wave | Staleness | Dep pressure | Design debt | Velocity | Coherence |

## Raw observations
<Cross-Wave patterns and surprises that don't fit the five signals.>
```

Depth follows heat. Commits and PRs are inputs, not outcomes: a correctly
dormant Wave is healthier than one with fifty commits and no finish lines.
Diagnose, don't prescribe: "this Wave needs design thinking" belongs here;
"split Task 3 and rewrite the README" doesn't.

Link each Task as `[identifier · Task title](URL) — status; next action/owner`,
from `task.identifier`, `task.name`, `reference.issue_url`, `runtime.status`
and `next_move.owner`. Never guess a URL; without one, keep the title and
status.
