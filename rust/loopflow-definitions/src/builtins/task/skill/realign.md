---
requires: intended outcome and work to reconcile
produces: plan, implementation and Wave memory that agree, with a short account of progress
action_style: procedural
---
Bring the plan and the code back into agreement with what the work has taught.

After a slice, or any stretch of work, answer three questions in order.

**What did the work teach?** Look at what the affected behavior does now.
Say what works, what doesn't fit, and which assumption in the plan turned out
wrong: mechanisms that proved unnecessary, behavior still missing or working
differently than intended. Include what moved around it: upstream changes,
related work, new decisions. Keep what you saw apart from why you think it
happened. Reuse existing results; run a check only when it settles a real
question. Realign starting is no reason to rerun tests.

**Is the plan still right?** Recover the intent first: the outcome wanted and
the constraints people stated, from the conversation, review feedback, plan
and code. When intent is missing, ask for the specific decision; the
implementation is not the spec. Then fix the plan at the source. Rewrite what
the work proved wrong and delete steps that no longer apply. What you learned
may change the approach, but missing behavior never weakens acceptance. A
change that needs a product decision is written as the exact open choice.
Nothing goes into the plan as settled unless someone said so; quote them.

Read the work once as a reviewer, one look at the shape and not a full
review: is there a concept, type or path it added that the design could do
without, or a simpler form of what it built? Write what you find into the
plan as remaining work. Correct small, clear mismatches in code, tests and
docs yourself; leave a substantial code change as remaining work.

**What is next?** Name the next slice: the one that teaches the most from
here, running end to end. If nothing remains, say so.

When you finish, the plan separates what exists from what remains, and the
code backs every claim of finished behavior. Keep blockers, contrary findings
and deferred checks where they affect the next action.

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

## Keeping notes readable

Only a little of `scratch/` is handed to each step; the rest is read on
demand, so what matters is that the plan is current and the notes aren't
misleading. Merge duplicates, drop notes inherited from a stacked parent, and
delete what a fresh read of the code would recover. Don't shrink scratch to a
size. What people said, word for word, stays: it can't be worked out again.

Wave memory is different: every Wave run pays for it. Run `lf context`, and
when it is over its limit retire the largest stale sections until it fits,
then stop. Commit notes before removing them. Never raise a limit to hide
overflow.

Finish briefly: what changed, what remains, any decision needed. Edit the plan
rather than adding a report. If everything already agrees, say so. Realign changes the
plan to match what was learned. It never adds behavior the plan didn't have:
that goes in `scratch/questions.md`.
