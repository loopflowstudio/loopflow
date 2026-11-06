---
requires: Linear Project plans and dated evidence, or an explicitly incomplete baseline
produces: one repository report and one scoped report per Wave
---
Review the repository's plans on dated evidence. Review never changes plans,
Linear Project status, Task state, KRs, or code. Return the complete report; a Task
worktree may also retain temporary review notes under `scratch/`.

Each Wave's plan is its one In Progress Linear Project. Planned and Completed
Linear Projects retain future and historical plans in Linear. Identify the
requested plans by name and each Wave's stable Linear Project ID; names or dates
alone never select between competing current Linear Projects. Preserve
available earlier reviews and dated observations. Union the evidenced starting
Wave roster with today's roster so retired, unavailable and new Waves remain
accounted for. Missing starting membership is an incomplete baseline.

Read current cached planning and Task conditions:

```bash
lf wave planning <wave> --json
lf wave show --json
```

Inspect historical Linear Project status/content and available issue history through a
read-only Linear interface, using stable IDs. The CLI status command has no
historical plan selector. If those records are inaccessible, name the missing
source; do not substitute today's plan. Do not run `wave sync` as a review
refresh: it can rename provider objects and apply legacy planning conversions.

Record exact KR wording, targets, source read dates and the interval actually
supported by evidence. Completed Linear Projects retain plans, but current issue
membership does not reconstruct Tasks moved at an earlier boundary. Current
metric readings are not historical measurements. Use dated observations with
their measurement windows and instrument revisions where available. Missing
target evidence is unknown; an evidenced omitted target is unset. Later shipment
cannot prove a promise inside an earlier interval.

Launch one bounded `lf -b --wave <name> wave-review-chapter "<plan name,
Linear Project ID, interval, exact starting KR ledger>"` per resolvable Wave. Supply
the available evidence and its gaps; there is no plan review tier. Retain
returned reports and launch references. A failed or unfinished child is
incomplete coverage, never success inferred from elapsed time. Account for
retired or unresolvable Waves from available dated records.

Verify every exact KR appears once with **holds**, **does not hold**, or
**unknown**. Recompute verdict totals from rows. Holds requires the full window
and denominator; a counterexample can refute a universal claim; missing or stale
evidence is unknown. Never promote an incomplete child into complete coverage.

Lead with experienced changes for users, operators, maintainers and agents,
organized by Wave. Preserve exact claims, evidence dates, counterexamples,
Task/PR references, changed claims and coverage gaps underneath. Report actual
Task transfers and cancellations only where dated evidence establishes them.
Nothing previews dispositions for the next plan: open work stays where it
is until someone moves it (`lf task move`), so report open Tasks as open.
Never invent a Task classifier in prose.

Separate next-plan proposals: valuable goals, changed assumptions, goals not
evidenced as priorities, and misplaced or unowned active work. Judge Wave
boundaries without creating Linear Projects or retiring unreviewed backlog. State
`Coverage: complete` or `Coverage: incomplete` for each Wave and the repository,
naming missing sources. Preserve corrections as dated observations. A review
with incomplete history cannot claim an accepted starting boundary.
