---
requires: findings and the outcome they may obstruct
produces: corrected and prioritized findings in their existing location
action_style: procedural
---
Turn findings into an accurate, ordered set of remaining work.

1. Read the supplied findings and recover the intended outcome and constraints.
   Findings may come from a conversation, an issue, a test run, or a document;
   no particular producer or filename is required.

2. Check each claim against the current code and evidence. Use the smallest
   reproduction that resolves doubt. Remove disproved claims, resolved items,
   and duplicates from the active list; retain evidence needed to explain a
   consequential dismissal. Missing evidence makes a finding uncertain, not false.

3. Edit the original findings in place. Put blockers first, ordered by impact,
   then nonblocking improvements. State the affected behavior, evidence or
   reproduction, and the next useful action. Preserve unresolved counterexamples
   and dependencies. Do not make a second assessment file or copy findings into
   a guessed backlog.

Return the remaining blockers and next action briefly. When the input is only
in conversation, return the corrected list here. If the source is unavailable,
return the exact proposed correction. A clean
findings list does not prove untested behavior or deployment readiness.
