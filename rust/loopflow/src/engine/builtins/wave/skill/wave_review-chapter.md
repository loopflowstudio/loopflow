---
requires: one Wave, a chapter name and Project ID where known, or a dated baseline
produces: one evidence-backed Wave chapter report
---
Review this Wave's chapter directly. Its current plan is one In Progress Linear
Project; Planned and Completed Projects retain future and historical plans.
There is no Project review tier. Review is read-only: never rotate, check KRs,
cancel Tasks, edit Projects or change Work state.

Read supplied dated reviews, this Wave's GOAL.md and MEMORY.md, and:

```bash
lf wave status <wave> --no-sync --json
lf roadmap --wave <wave> --json
```

These commands describe cached current planning and Task conditions. Inspect
requested historical Project status/content and available issue history by
stable Project ID through a read-only Linear interface. The status command has
no historical chapter selector. Do not run `wave sync` during review: it can
rename provider objects and convert legacy planning. If a source is unavailable
or stale, record that limit rather than repairing it or substituting today's plan.

Preserve exact KR wording, targets, dated observations, measurement windows and
instrument revisions applicable to the reviewed interval. Completed Projects
retain plans, but their current membership cannot reconstruct Tasks moved away
at an earlier boundary. A moved Task is neither completed nor abandoned. Work
shipped later cannot prove an earlier KR. Bound the interval only from available
evidence; absent dates or starting membership leave an incomplete baseline.

Give every exact KR one verdict: **holds**, **does not hold**, or **unknown**.
Holds requires the complete observation window and denominator. A dated
counterexample refutes a universal claim. Missing or stale evidence is unknown.
A checked box, PR, demo or single successful run alone cannot prove the promised
outcome. Record changed or removed claims separately.

The Wave owns instruments and observations; Project content owns KRs, Tasks and
targets. An evidenced omitted target is unset, not inherited or missed. Missing
historical target evidence is unknown. Rotation does not copy readings, so use
retained dated evidence and never evaluate an old promise with today's readings,
targets or instrument definition.

Lead with what improved for users, operators, maintainers or agents and the few
facts supporting it. Separate observations from interpretation. Include exact
claims, evidence dates, verdicts, Task/PR links and gaps. Recompute totals from
KR rows. State `Coverage: complete` or `Coverage: incomplete` with missing scope
and sources.

Report historical Task transfers and cancellations only when evidenced. If the
parent supplies a dated repository rotation preview, report this Wave's proposed
dispositions separately; they are not historical facts or permission to apply.
Never implement a second classifier. Keep next-chapter recommendations separate
from observations, including work lacking evidence of priority.

Return the complete report and evidence dates. The repository reviewer combines
Wave reports and may keep temporary notes in scratch. Preserve earlier reports
and append dated corrections. No fabricated acceptance or live mutations belong
in review.
