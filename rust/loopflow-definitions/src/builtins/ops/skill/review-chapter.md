---
requires: Linear Project plans and dated results, or a baseline marked incomplete
produces: one repository report and one report per Wave
---
Review how the repository's chapter went against what was promised, Wave by Wave, and return the report.

Review changes nothing: no plans, Project status, Tasks, KRs or code.

A chapter is the shared name of each Wave's In Progress Linear Project.
Identify the chapter by name and each Wave's Project by ID. Cover the Waves at
the start of the chapter and today's, so retired and new Waves both appear. If
the starting roster is missing, the baseline is incomplete; say so.

```bash
lf wave plan <wave> --json
lf wave show --json
```

These show today. Read past Project status, content and issue history from
Linear, read-only, by ID. If those records are out of reach, name the missing
source instead of using today's plan. Don't run `wave sync` here; it can
rename Linear objects.

**Judge the past by the past.** Record each KR's exact wording, its targets,
when you read each source, and the interval the data covers. Today's
membership doesn't show Tasks moved earlier, and today's readings aren't past
measurements. A target with no data is unknown; one left out on purpose is
unset. Shipping later doesn't meet a promise for an earlier interval.

Launch one `lf -b --wave <name> wave-review-chapter "<chapter name, Project ID, interval, exact starting KRs>"`
per Wave you can resolve, and keep each report. A failed or unfinished child
is missing coverage, never success.

Every KR appears once as **holds**, **does not hold** or **unknown**. Holds
needs the full window and denominator; one counterexample breaks a universal
claim; missing or stale data is unknown. Recount the totals from the rows.

Lead with what changed for the people using and running the product, by Wave.
Under that, the exact claims, dates, counterexamples, Task and PR links and
gaps. Open work stays where it is until someone moves it, so report open Tasks
as open.

Keep next-chapter proposals apart: goals worth pursuing, assumptions that
changed, goals that weren't really priorities, work in the wrong Wave. End
each Wave and the repository with `Coverage: complete` or
`Coverage: incomplete`, naming the missing sources.
