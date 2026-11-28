---
requires: intended outcome and work to reconcile
produces: current plan with any remaining implementation work
action_style: procedural
---
Update the plan with what the work has taught and what still needs to change.

**What did the work teach?** Compare the intended behavior with what you
observed. Include upstream changes, related work and new decisions. Distinguish
what happened from why you think it happened. Reuse existing results. Run a
check only to settle an open question.

With a fresh `scratch/concept-review.html`, work from its findings. Before
implementation, the review itself is what was learned. Without a fresh review,
look once for a simpler shape or a concept, type or path the design can drop.

**Is the plan still right?** Recover the outcome and constraints from the
conversation, review feedback, plan and code. Ask for the specific decision
when intent is missing. Correct mistaken assumptions in `scratch/plan.md`
and delete steps that no longer apply. Keep promised behavior that is still
missing. Quote what people decided and name any choice still open.

Give each finding a place:

- Correct the plan and any clear user-docs mismatch under `docs/` or in a
  `README.md`. Record the docs fix in the plan.
- Queue every other in-scope change for `implement`, including small code or
  test fixes, conceptual rework and missing promised behavior.
- List separate outcomes under `## Deferred findings` with what each would
  take. `ship-review` reads them there. Put new ideas in `scratch/questions.md`.
- For a reversible design choice in a headless run, use the recommended
  direction. Mark it for review in the plan with the alternative and why you
  passed it over, so later steps build one design and `ship-review` surfaces
  the choice.

Record each review finding by substance so later runs can skip findings the
plan already answers. When a fresh review arrives, replace settled review
notes and keep `## Deferred findings`.

**What is next?** Group remaining work into end-to-end passes, aiming for five
or fewer across the Task. Name what the next pass delivers and what it will
teach. Keep blockers, contrary findings and deferred checks beside the work
they affect. Write the smallest plan continued work needs if none exists.

## Deciding a loop

Only a step whose message says it decides a loop returns a verdict:

- Return `blocked` if repeated passes fail to converge on an in-scope fix or
  progress needs a design choice that can't be undone once shipped. Say why.
- Return `iterate` for any remaining in-scope implementation fix. Name the
  next pass so `implement` makes the fix and `concept-review` reviews it.
- Return `advance` when no implementation work remains.

Any other run (`plan`, `refresh`, standalone, or a step outside the loop)
reports what remains and names no verdict, in the reply or the plan. Record
any open irreversible choice in both `scratch/questions.md` and the plan.

## Keeping notes readable

Keep the plan current so later steps can tell what exists and what remains.
Check that the code backs each claim of finished behavior. Keep people's
exact words, run results, outside facts and the few findings that still
explain the approach, with source paths. Merge duplicates, drop notes
inherited from a stacked parent and cut what a fresh read could recover.

When the Wave's memory is this step's to write, curate it as
`lf help wave-session` describes.

Edit the plan directly. Finish briefly with what changed, what remains and
any decision needed. If everything already agrees, say so.
