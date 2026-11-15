---
requires: findings and the outcome they may block
produces: corrected, prioritized findings where they already live
action_style: procedural
---
Turn findings into an accurate, ordered list of remaining work.

1. Read the findings and work out the intended outcome and its constraints.

2. Check each claim against the current code. Run the smallest reproduction
   that settles doubt. Drop disproved claims, fixed items and duplicates; keep
   a line on why when dismissing something important. A finding nobody could
   check is uncertain, not false.

3. Edit the findings in place. Blockers first, ordered by impact, then
   nonblocking improvements. For each: the affected behavior, the reproduction,
   and the next action. Keep open counterexamples and dependencies. Don't write
   a second assessment file or copy findings into a guessed backlog.

Return the remaining blockers and next action briefly. When the findings exist
only in conversation, return the corrected list here; when their source is
unreachable, return the exact correction. A clean list says nothing about
untested behavior or whether it's ready to deploy.
