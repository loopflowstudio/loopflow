---
requires: scratch/ux-research/loop-NN/evaluations.md
produces: scratch/ux-research/design-guidelines.md
---
Fold this loop's results into the persistent, accumulating artifacts so the next
loop starts smarter than this one did. This is the fold that makes the loop a
loop and not a one-off.

## Update three durable files

**1. `scratch/ux-research/design-guidelines.md`** — what we now believe about
Loopflow's UX. Append this loop's learnings as dated entries. A good guideline
is a claim we'd defend and design against next time (e.g. "Status must carry its
*reason*, not just a word"). Cite the loop and the evidence. If this loop
*contradicts* an earlier guideline, don't delete it silently — mark it revised
and say why. Guidelines are the loop's compounding memory.

**2. `scratch/ux-research/questions.md`** — resolve, sharpen, or add:
- Move questions the evaluations *answered* into a "Resolved" section with the
  answer.
- Sharpen questions the evaluations reframed.
- Add new questions the candidates exposed.
- Update the **target-behavior backlog**: the tension this loop surfaced usually
  spawns the next behavior to test. Add it, ordered.

**3. The loop folder is already written** (proposal / candidates / evaluations).
Leave it as the immutable record of this pass.

## Then decide the next move

At the end of `design-guidelines.md`'s new entry, state in one line what the
*next loop* should probably test, so `propose` has a running start. Don't pick a
winner if the tension is genuinely unresolved — record it as an open decision a
human should make, and design the next loop to gather the evidence that would
settle it.

## Output

Updated `design-guidelines.md` and `questions.md` (durable, growing), plus a
one-paragraph fold summary in your final report: what we learned, what's now
open, what the next loop targets.
