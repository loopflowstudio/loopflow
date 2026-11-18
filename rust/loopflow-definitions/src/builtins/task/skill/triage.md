---
requires: findings and the outcome they may block
produces: corrected, prioritized findings where they already live
action_style: procedural
---
Turn findings into an accurate, ordered list of remaining work.

1. **Read the findings** and work out the outcome they may block.

2. **Check each claim against the current code.** Run the smallest
   reproduction that settles doubt. Drop disproved claims, fixed items and
   duplicates, keeping a line on why when you dismiss something important. A
   finding nobody could check is uncertain, not false.

3. **Edit the findings where they live.** Blockers first, ordered by impact,
   then improvements. For each: the affected behavior, the reproduction and
   the next action. Keep open counterexamples and dependencies. No second
   assessment file, and no copying findings into a guessed backlog.

Return the remaining blockers and the next action. Findings that exist only in
the conversation get the corrected list here. A clean list says nothing about
untested behavior.
