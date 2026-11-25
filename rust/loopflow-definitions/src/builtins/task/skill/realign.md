---
requires: intended outcome and work to reconcile
produces: plan and implementation that agree, with a short account of progress
action_style: procedural
---
Bring the plan and the code back into agreement with what the work has taught.

After a slice, a review, or upstream changes, answer three questions in order.
Before anything is built, the review is what was learned.

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
change that needs a design decision is written as the exact open choice.
Nothing goes into the plan as settled unless someone said so; quote them.

Read the work once as a reviewer, one look at the shape and not a full
review: is there a concept, type or path it added that the design could do
without, or a simpler form of what it built? Write what you find into the
plan as remaining work. Correct small, clear mismatches in code, tests and
docs yourself; leave a substantial code change as remaining work.

**What is next?** Regroup what remains into end-to-end passes, aiming for
five or fewer across the Task. Name what the next pass will deliver and
what it will teach. If nothing remains, say so.

When you finish, the plan separates what exists from what remains, and the
code backs every claim of finished behavior. Keep blockers, contrary findings
and deferred checks where they affect the next action.

The plan says what will be built and what remains. Keep what can't be worked
out again: what people said, word for word, what a run showed, facts from
outside the repository, and the few findings that still explain the approach,
with source paths. Cut the rest. With no plan, write the smallest one continued
work needs.

## After a review

When `scratch/concept-review.html` has findings the plan has not answered, give
each one a place:

- **Small, clear mismatch:** fix it here, in plan, code, tests or docs.
- **Conceptual rework in the area this work touches, or missing promised
  behavior:** write it into the plan as the next pass.
- **Separate outcome:** list it in the plan under `## Deferred findings` (which
  `ship-review` reads), with what it would take. Don't build it.
- **Reversible design choice (headless):** adopt the recommended direction
  into the plan marked as made for review, with what was passed over and why
  (never as settled), so downstream steps build one coherent design and
  `ship-review` surfaces the choice.

Record in the plan what happened to each finding by substance (not bare
numbers), so a later `realign` (such as `refresh`) does not act on the same
page twice. When a fresh `scratch/concept-review.html` arrives (for example,
after `ship-review` iterates back to `build`), replace earlier settled review
notes while keeping `## Deferred findings` intact.

## Deciding a loop

When this step is deciding a loop: return `iterate` only when the plan now
holds a pass of rework or missing promised behavior, naming that pass in the
summary; return `advance` when what was found is fixed here or deferred; return
`blocked` with the question on a design choice that can't be undone once
shipped.

Outside a loop (`plan`, `refresh`, or standalone), there is no `blocked`
verdict: record any open irreversible choice in `scratch/questions.md` and
`scratch/plan.md`, report what changed, and finish.

## Keeping notes readable

Only a little of `scratch/` is handed to each step; the rest is read on
demand, so what matters is that the plan is current and the notes aren't
misleading. Merge duplicates, drop notes inherited from a stacked parent, and
delete what a fresh read of the code would recover. Don't shrink scratch to a
size. What people said, word for word, stays: it can't be worked out again.

When the Wave's memory is this step's to write, curate it as
`lf help wave-session` describes.

Finish briefly: what changed, what remains, any decision needed. Edit the plan
rather than adding a report. If everything already agrees, say so. Realign
changes the plan to match what was learned. It never adds behavior the plan
didn't have: that goes in `scratch/questions.md`.
