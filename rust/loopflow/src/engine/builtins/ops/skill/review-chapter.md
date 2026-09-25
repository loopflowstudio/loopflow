---
requires: repository chapter record or an explicitly incomplete baseline
produces: one repository report and one scoped report per Wave
---
Review the repository's chapter on dated evidence. Review never changes plans,
Task state, KRs, or code. A Task worktree may write the chapter archive; otherwise
return the complete report in conversation.

Use the newest merged `.lf/chapters/<id>/start.md` as the accepted publication
boundary. A live chapter ahead of that archive stays explicitly pending
publication. Union the starting Wave ledger with the current roster so retired,
unavailable, and newly added Waves remain accounted for. Preserve exact chapter
IDs, KR claims, dates, and application receipts.

Read `lf status <wave> --chapter <id> --json` for historical content and
membership, and `lf roadmap --json` for current Task conditions. An old chapter's
snapshot stays true after active Tasks move and later ship. Later completion
cannot prove a promise inside an earlier interval. Use the snapshot's frozen
metric targets and readings, dated by metrics_evaluated_at. A missing target is
unset, not missed; do not reevaluate old chapters with current targets or
instrument files. If snapshots or lineage are
missing, report an incomplete baseline instead of inventing dates or silently
sampling the current plan.

Launch one bounded `lf -b --wave <name> wave/review-chapter "<chapter id,
interval, exact starting KR ledger>"` per resolvable Wave. There is no Project
review tier. Archive final reports and Run IDs. Recover lost launch output with
`lf runs --parent "$LF_RUN_ID" --json` and `lf runs <id> --final`. A failed or
unterminated child is incomplete coverage; never infer success from elapsed time.
Account for retired/unresolvable Waves directly from their dated archive.

Verify every exact KR appears once with **holds**, **does not hold**, or
**unknown**. Recompute verdict totals from rows. Holds requires the full window
and denominator; a counterexample can refute a universal claim; missing or stale
evidence is unknown. Never promote an incomplete child into complete coverage.

Lead the report with experienced changes for users, operators, maintainers and
agents, organized by Wave. Underneath, preserve exact claims, evidence dates,
counterexamples, Task/PR references, and coverage gaps. Include the deterministic
Task dispositions supplied by each chapter snapshot: started work will move,
untouched backlog will expire, and completed work stays historical. Review must
not implement another classifier or apply those dispositions.

Separate proposals for the next chapter: valuable goals, changed assumptions,
goals not evidenced as priorities, and misplaced or unowned active work. Judge
Wave boundaries without creating new Projects or preserving old backlog by
inertia. State `Coverage: complete` or `Coverage: incomplete` for every Wave and
the repository, naming missing sources.

Write `.lf/chapters/<id>/review.md` and scoped Wave reports when in the Task
worktree, and return the readable report. Existing reports receive dated
corrections, never rewritten observations. A baseline has no `start.md` and
cannot claim an accepted chapter boundary.
