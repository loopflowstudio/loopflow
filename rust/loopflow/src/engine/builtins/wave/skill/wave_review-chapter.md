---
requires: one Wave and the exact chapter id or a dated baseline
produces: one evidence-backed Wave chapter report
---
Review this Wave's chapter directly. There is one plan and no Project review tier.
Review is read-only: never rotate, check KRs, close Tasks, or change Work state.

Read the accepted start record, this Wave's GOAL.md and MEMORY.md, and:

```bash
lf status <wave> --chapter <id> --json
lf roadmap --wave <wave> --json
```

The dated chapter snapshot owns historical membership and wording. Its
`observed_at` and `closed_at` bound the facts. Current Task status supplements
them; it cannot rewrite the earlier interval. A moved Task is neither completed
nor abandoned. Work shipped after the interval cannot prove an earlier KR.
For an open chapter, refresh PM explicitly if needed and record that read's date.
If no chapter snapshot exists, reconstruct a baseline from dated records and
mark the missing lineage. Never invent a complete history from the current plan.

Give every exact KR one verdict: **holds**, **does not hold**, or **unknown**.
A holds verdict requires the complete observation window and denominator.
A dated counterexample refutes a universal claim. Missing or stale evidence is
unknown. A checked box, PR, demo, or single successful run is not by itself
proof of the promised outcome. Record changed or removed claims separately.

Lead with what improved for users, operators, maintainers, or agents and the
few facts supporting it. Keep observed facts separate from interpretation.
Include exact KR wording, dated observations, verdicts, Task/PR links, and gaps.
Recompute totals from KR rows. State `Coverage: complete` or `Coverage:
incomplete` with the missing scope and source.

Use the API's Task dispositions in the snapshot to report active work that will
move and untouched backlog that will expire. Do not implement a second
classifier in prose. Separate those facts from recommendations about the next
objectives, what was learned, and work that lacks evidence of priority.

Return the complete report and evidence dates. The repository review archives
one report per Wave. No child Project Runs, fabricated acceptance, or live
mutations belong in review.

The Wave owns the enduring objective and measurement instruments. The chapter
owns KRs, Tasks, and targets referring to those instruments by metric_id.
Never author a separate chapter objective or a target in a Wave instrument.
An omitted target is unset; it is not copied from the previous chapter.
Review closed chapters using their frozen metric readings and
metrics_evaluated_at, not today's readings or newly edited instrument files.
