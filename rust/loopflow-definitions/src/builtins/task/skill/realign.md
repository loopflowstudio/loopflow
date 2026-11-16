---
requires: intended outcome and work to reconcile
produces: plan, implementation and Wave memory that agree, with a short account of progress
action_style: procedural
---
Bring the plan and the code back into agreement with what the work has taught.

1. **Recover the intent.** The outcome wanted, the constraints people stated
   and the current approach, from the conversation, review feedback, plan and
   code. Look at what the affected behavior does now. When intent is missing,
   ask for the specific decision; the implementation is not the spec.

2. **Find what moved.** Upstream changes, related work, new decisions. Then
   where the work changed our understanding: assumptions that failed,
   mechanisms that turned out unnecessary, behavior still missing or working
   differently than intended. Keep what you saw apart from why you think it
   happened. Reuse existing results; run a check only when it settles a real
   question. Realign starting is no reason to rerun tests.

3. **Fix it at the source.** Rewrite stale parts of the plan, delete obsolete
   steps, and bring remaining work up to date. Correct small, clear mismatches
   in code, tests and docs; leave a substantial code change as remaining work.
   What you learned may change the approach, but missing behavior never
   weakens acceptance. A change that needs a product decision is written as
   the exact open choice. Nothing goes into the plan as settled unless someone
   said so; quote them.

4. **Check the result.** The plan separates what exists from what remains, and
   the code backs every claim of finished behavior. Keep blockers, contrary
   findings and deferred checks where they affect the next action.

The plan says what will be built and what remains. Keep what can't be worked
out again: what people said, word for word, what a run showed, facts from
outside the repository, and the few findings that still explain the approach,
with source paths. Cut the rest. With no plan, write the smallest one continued
work needs.

## Wave memory

`.lf/wave/` is read-only unless the running Graph's step maintains it; otherwise
put proposed changes in `scratch/`. When it is yours to maintain, read the
Wave's `MEMORY.md` and its immediate children's, promote lessons that apply
across the parent, and write the direction and lessons this work produced:
what future Wave work should know, not a copy of this pass. Replace stale
guidance and remove duplication. Curate the selected Wave, not its ancestors.

## Context budget

Run `lf context`. Bring over-budget scratch under its limits: merge
duplicates, summarize, drop notes inherited from a stacked parent, and leave
history to git. For Wave memory, retire the largest stale sections until it
fits, then stop; don't shrink what is already in budget. Commit notes before
removing them. Never raise a limit to hide overflow.

Finish briefly: what changed, what remains, any decision needed. Edit the plan
rather than adding a report. If everything already agrees, say so. Give the
next step the facts to judge with; the decision to advance is not yours.
