---
requires: a chosen direction, an existing design, or a Task brief
produces: an implementation plan with risks answered, one approach chosen and the demo named
---
Turn the direction into a plan worth building: risks answered, one approach chosen, the demo named.

The product direction arrives settled: the Task brief, the direction chosen in
explore, what people have said. Plan that outcome. Don't reopen discovery or
swap in something technically convenient. Continue an existing design instead
of re-deriving it, and if the plan is already good enough, keep it. Name an
open product choice that blocks planning and ask; a plan can't grant approval.

## Method

1. **Read the intent, then the system.** The brief, `scratch/` and the agent
   guide say what is wanted. Then follow the affected behavior through its
   docs, code, callers and tests until you know what constrains this change.

2. **De-risk.** Find what could sink the approach and answer it: read the docs,
   call the API, run the probe. Start with risks already flagged, then look for
   the ones nobody named. Come back with answers, not a worry list: "Linear's
   API has no conditional assignment, so we read-then-assign with conflict
   detection", not "there might be API limitations". When two explanations
   fit, run the smallest probe that tells them apart. Keep the finding even
   when it kills the attractive approach.

3. **Compare mechanisms.** Where it's uncertain, weigh approaches that differ
   by mechanism, not presentation. A route is blocked when what it still needs
   is as hard as the original problem.

4. **Imagine it shipped.** Wild success: people love it. What details made it
   great? Wild failure: six months on you're ripping it out. What went wrong?

5. **Choose.** Commit to one direction. Make reversible choices yourself and
   leave real product decisions open and named.

6. **Name the demo.** The command someone runs and what appears, or the
   interaction that now works. If you can't describe it, the slice stops one
   step short; carry it to where it shows itself.

7. **Write the plan** to `scratch/<slug>.md`. Every other scratch file is input
   to reconcile; an older, worse design doesn't win by being there first.

## What the plan covers

Cover these where they matter and skip the rest:

- **User-visible outcome** — whose behavior changes and what they see.
- **Acceptance check at gate** — one scenario through the source of truth and
  every affected consumer, as a headless command and its expected result.
- **Source of truth** — the record, model or API everything else derives from.
- **Affected surfaces and consumers** — every CLI, wire type, app or reader
  that changes or must keep working.
- **Absent and error states** — what empty, missing, invalid or failed means
  at each consumer.
- **Operational boundary** — latency, scale, network or recovery limits the
  code must hold.
- **Exclusions** — nearby behavior left out.
- **Delete — do not maintain** — the files, symbols, tests and fixtures this
  change removes, and the behavior and data that must survive. Never plan
  repairs to them. Add earlier attempts you find while reading.
- **Slices** — for a change that can't be split, the full end state, deepest
  deletions first, with the current slice marked and updated in place.

"Done when" names behavior people see, not a list of files. Keep each finding
beside the decision it informs, with source paths. Quote what people said word
for word; the plan describes the work and gives its reader no orders.

In a Wave, derive "Done when" from its KRs and check its `MEMORY.md` for
decisions already made.

## Principles

**Bold over safe.** When unsure, pick the more ambitious option.

**Concrete over abstract.** "Fast" means nothing. "P95 under 100ms" means
something.

**Whole over staged.** Land a coherent architectural change at once. Splitting
it breeds adapters and dual states: legacy/new splits, fallbacks and dual
writes need a reason and a deletion date. Split only when each piece is
noticeable on its own.

**Reshape over layer.** Map the concepts, types, writers and launch paths that
exist before adding another.
