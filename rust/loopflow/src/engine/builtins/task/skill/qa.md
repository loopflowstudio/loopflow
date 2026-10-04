---
requires: a change or behavior to assess
produces: verified repairs and unresolved findings with supporting evidence
action_style: procedural
---
Find defects that matter to the intended experience and resolve what the evidence makes clear.

Read the supplied change, design, existing findings and repository testing guidance.
Recover observable success, affected consumers and plausible near-misses. Assess
the actual change, including unfinished work; do not require a branch or a report
from a particular preceding skill.

Read planning context when the seed names the exact wave or Task, or supplies
a concrete coordination question. Do not infer a Wave or repair PM access as a
prerequisite to assessing the change.

1. Inspect the affected paths and derive adversarial cases from their contract:
   absent or malformed input, boundaries, concurrency, partial failure and recovery
   where relevant. Follow behavior across all consumers the change claims to cover.
2. Reuse applicable passing evidence for unchanged content. Run the smallest
   additional checks that distinguish a defect from a hypothesis. Expand to
   affected suites when the boundary requires them, not a full matrix by ritual.
   For user-facing changes, inspect the actual experience as well as the source.
3. Preserve counterexamples and revise the explanation when a check contradicts
   it. Record reproduction, consequence and evidence; distinguish defects from
   uncertain concerns and optional improvements.
4. Repair clear defects within the task scope and verify the original
   failure plus relevant regressions. When asked for an independent audit or
   read-only review, leave fixes as findings. A consequential design change
   remains a scope decision for the participant.
5. Update findings at their existing owner. Keep unresolved issues, exact blockers
   and evidence limits where the next worker can use them. Write a separate report
   only when its reader needs one; no findings means no invented report.

Finish with the experienced result, checks and their limits, and remaining issues
ordered by consequence. Passing checks do not prove missing behavior or configured
acceptance. Do not publish, land or select Flow navigation from an assessment.
