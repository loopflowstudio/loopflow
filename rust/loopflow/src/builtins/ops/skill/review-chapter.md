---
requires: Linear Project plans and dated results, or a baseline marked incomplete
produces: one repository report and one report per Wave
---
Review how the repository's chapter went against what was promised, Wave by Wave, and return the report.

Review changes nothing: no plans, Project status, Tasks, KRs or code. Temporary
notes may go in `scratch/`.

A chapter is the shared name of each Wave's In Progress Linear Project; Planned
and Completed Projects hold future and past plans. Identify the chapter by name
and each Wave's Project by ID; a name or date never picks between two current
Projects. Use earlier reviews and dated observations. Cover the Waves at the
start of the chapter and today's, so retired, unavailable and new Waves all
appear. If the starting roster is missing, the baseline is incomplete.

Read current planning and Task state:

```bash
lf wave plan <wave> --json
lf wave show --json
```

Read past Project status, content and issue history through Linear, read-only,
by ID. The CLI has no past-chapter selector. If those records are out of reach,
name the missing source instead of using today's plan. Don't run `wave sync`
here: it can rename Linear objects and convert old plans.

Record each KR's exact wording, its targets, when you read each source, and the
interval the data actually covers. Today's issue membership doesn't show Tasks
moved earlier, and today's metric readings aren't past measurements; use dated
observations with their windows and instrument versions. A target with no data
is unknown; one that was left out on purpose is unset. Shipping later doesn't
meet a promise for an earlier interval.

Launch one `lf -b --wave <name> wave-review-chapter "<chapter name, Project ID, interval, exact starting KRs>"`
per Wave you can resolve, with the data and its gaps. There is no Project-level
review. Keep each returned report. A failed or unfinished child is missing
coverage, never success. Account for retired or unresolvable Waves from dated
records.

Check that every KR appears once as **holds**, **does not hold** or **unknown**,
and recount the totals from the rows. Holds needs the full window and
denominator; one counterexample can break a universal claim; missing or stale
data is unknown.

Lead with what changed for the people using and running the product, by Wave.
Under that, keep exact claims, dates, counterexamples, Task and PR links, changed
claims and gaps. Report Task moves and cancellations only where dated records
show them. Nothing previews where
Tasks go next: open work stays where it is until someone moves it (`lf task move`),
so report open Tasks as open. Don't sort Tasks yourself in prose.

List next-chapter proposals separately: goals worth pursuing, assumptions that
changed, goals that weren't really priorities, and active work in the wrong Wave
or with no one on it. Judge Wave scope without creating Projects or retiring
unreviewed backlog. End each Wave and the repository with `Coverage: complete` or
`Coverage: incomplete`, naming missing sources. Record corrections as dated
observations.
