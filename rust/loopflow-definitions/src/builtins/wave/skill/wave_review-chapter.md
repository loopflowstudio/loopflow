---
requires: one Wave, a chapter name and Project ID where known, or a dated baseline
produces: one Wave chapter report with a dated verdict per KR
---
Review this Wave's chapter: what improved, and whether each KR holds.

Review only reads: never replan, check KRs, cancel Tasks, edit Projects or
change Task state.

Read any dated reviews supplied, the Wave's `GOAL.md` and `MEMORY.md`, and:

```bash
lf wave plan <wave> --json
lf wave show <wave> --json
```

These show today's planning. For past Projects, read status, content and issue
history from Linear, read-only, by Project ID. Don't run `wave sync`; it can
rename provider objects. If a source is unavailable or stale, note it; don't
repair it or substitute today's plan.

**Judge the past by the past.** Quote KR wording, targets, dated readings,
measurement windows and instrument versions as they applied in the interval.
Today's membership can't rebuild Tasks moved away earlier; a moved Task is
neither completed nor abandoned. Work shipped later doesn't satisfy an earlier
KR. Missing dates or starting membership make the baseline incomplete.

Give each KR one verdict: **holds**, **does not hold** or **unknown**. Holds
needs the full measurement window and denominator. One dated counterexample
breaks a universal claim. Missing or stale readings mean unknown. A checked
box, a PR, a demo or one good run doesn't show the promised outcome. A target
left out on the record is unset, not missed.

Lead with what improved for users, operators, maintainers or agents, and the
few facts behind it. Keep observation apart from interpretation, and
next-chapter recommendations apart from both. Include exact claims, dates,
verdicts, Task and PR links and gaps. Recount totals from the KR rows, and
state `Coverage: complete` or `Coverage: incomplete` with what is missing.

Return the full report with its dates. Leave earlier reports as they are and
append dated corrections.
