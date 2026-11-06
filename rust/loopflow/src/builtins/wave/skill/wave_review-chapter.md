---
requires: one Wave, a chapter name and Project ID where known, or a dated baseline
produces: one Wave chapter report with a dated verdict per KR
---
Review this Wave's chapter: what improved, and whether each KR holds.

The Wave's local binding selects the current Project; Planned and Completed
Projects hold future and past plans. Review only reads: never replan, check KRs,
cancel Tasks, edit Projects or change Task state.

Read any dated reviews supplied, the Wave's GOAL.md and MEMORY.md, and:

```bash
lf wave planning <wave> --json
lf wave show <wave> --json
```

These show cached current planning and Tasks. For past Projects, read status,
content and issue history by stable Project ID through a read-only Linear
query; the status command has no past-chapter selector. Don't run `wave sync`
during review: it can rename provider objects and convert old planning. If a
source is unavailable or stale, note it; don't repair it or substitute today's
plan.

Quote KR wording, targets, dated readings, measurement windows and instrument
versions exactly as they applied in the interval. Completed Projects keep their
plans, but today's membership can't rebuild Tasks moved away earlier. A moved
Task is neither completed nor abandoned. Work shipped later doesn't satisfy an
earlier KR. Bound the interval only from what you can read; missing dates or
starting membership make the baseline incomplete.

Give each KR one verdict: **holds**, **does not hold**, or **unknown**. Holds
needs the full measurement window and denominator. One dated counterexample
breaks a universal claim. Missing or stale readings mean unknown. A checked box,
PR, demo or single good run doesn't show the promised outcome. List changed or
removed claims separately.

The Wave owns instruments and readings; the Project owns KRs, Tasks and targets.
A target left out on the record is unset, not inherited or missed; a target with
no record is unknown. Plans don't copy readings, so use dated readings from
the time and never judge an old promise by today's readings, targets or
instrument.

Lead with what improved for users, operators, maintainers or agents and the few
facts behind it. Keep observations apart from interpretation. Include exact
claims, dates, verdicts, Task and PR links, and gaps. Recompute totals from the
KR rows. State `Coverage: complete` or `Coverage: incomplete` with what's
missing.

Report past Task moves and cancellations only when the record shows them. Keep next-chapter recommendations apart from
observations, including work with no sign of priority.

Return the full report with its dates. The repository review combines Wave
reports and may keep notes in `scratch/`. Leave earlier reports as they are and
append dated corrections. Review never claims acceptance or changes live state.
