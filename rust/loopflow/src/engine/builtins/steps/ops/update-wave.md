---
requires: diff vs main, wave/<wave>/
produces: wave/<wave>/ (updated)
---
Revise the wave plan based on what we just shipped.

## Goal

After shipping a wave item, the plan should reflect what we learned. A wave plan written before building is a hypothesis. This step updates the hypothesis with evidence.

## Workflow

1. Read the diff — what was actually built?
2. Read the remaining wave items in `wave/<wave>/`
3. Read `wave/<wave>/README.md` for strategic context
4. Compare what was built to what was planned. Note surprises — things that were harder, easier, or different than expected.
5. Update the wave plan.

## What to look for

**Assumptions that broke.** The plan assumed X, but building revealed Y. Does this change what comes next?

**Scope that shifted.** Did the shipped item end up bigger or smaller than planned? Does that compress or expand later phases?

**New questions.** Did building this surface questions we didn't have before? Add them to the relevant phase.

**Resolved questions.** Did building this answer open questions from later phases? Update them.

**Sequence changes.** Given what we know now, is the ordering still right? Should something move earlier because it's more uncertain than we thought? Should something be dropped because building this made it unnecessary?

## What to update

- **Roadmap** — update phase status, revise scope based on what we learned
- **Risks** — add new risks discovered during implementation, resolve answered questions
- **Goals** — refine success criteria if they evolved, update invariants if new ones emerged
- **Metrics** — note any observable signals from what we shipped
- **Vision** — should rarely change. If it does, flag it explicitly — vision drift is a design decision, not a side effect

## What not to do

- Don't rewrite phases that are still far out. Update the next 1-2 phases; leave distant ones alone.
- Don't remove open questions just because they're uncomfortable. If we still don't know, say so.
- Don't add items just because you see opportunities. This step maintains the plan, it doesn't expand scope.

## Output

Updated files in `wave/<wave>/`. Commit the changes with a message describing what shifted and why.

If nothing changed — the plan still holds — write a brief note in the commit: "wave: reviewed after <item>, no changes needed."
