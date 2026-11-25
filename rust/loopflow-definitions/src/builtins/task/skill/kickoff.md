---
requires: a chosen direction, an existing design, or a Task brief
produces: an implementation plan with risks answered, one approach chosen and the demo named
---
Turn the direction into a plan worth building: risks answered, one approach chosen, the demo named.

**Plan the outcome you were given.** The direction arrives settled: the Task
brief, what was chosen in explore, what people have said. Don't reopen
discovery or swap in something technically convenient.

**Continue; don't re-derive.** Build on the design that exists. If the plan
is already good enough, keep it.

**Choose.** An open design choice gets asked when someone is here. With
nobody here, make the bold choice and mark it in the plan as made for review,
with what you passed over.

## Method

1. **Read the intent, then the system.** The brief, `scratch/` and the agent
   guide say what is wanted. Then follow the affected behavior through its
   docs, code, callers and tests until you know what constrains this change.

2. **De-risk.** Find what could sink the approach and answer it: read the docs,
   call the API, run the probe. Start with risks already flagged, then look for
   the ones nobody named. Come back with answers, not a worry list: "the
   tracker's API has no conditional assignment, so we read-then-assign with
   conflict detection", not "there might be API limitations". When two
   explanations fit, run the smallest probe that tells them apart. Keep the
   finding even when it kills the attractive approach.

3. **Compare mechanisms.** Where it's uncertain, weigh approaches that differ
   by mechanism, not presentation. A route is blocked when what it still needs
   is as hard as the original problem.

4. **Imagine it shipped.** Wild success: people love it. What details made it
   great? Wild failure: six months on you're ripping it out. What went wrong?

5. **Commit to one direction.** The plan describes one design, not a menu.

6. **Name the demo.** The command someone runs and what appears, or the
   interaction that now works. If you can't describe it, the slice stops one
   step short; carry it to where it shows itself.

7. **Write the plan** to `scratch/plan.md`, the one file every later step is
   handed first. Every other scratch file is input to reconcile; an older,
   worse design doesn't win by being there first.

## What the plan covers

Cover these where they matter and skip the rest:

- **User-visible outcome** — whose behavior changes and what they see.
- **Acceptance check** — one scenario through the source of truth and
  every affected consumer, as a headless command and its expected result.
- **Source of truth** — the record, model or API everything else derives from.
- **Affected surfaces and consumers** — every CLI, wire type, app or reader
  that changes or must keep working.
- **Absent and error states** — what empty, missing, invalid or failed means
  at each consumer.
- **Operational boundary** — latency, scale, network or recovery limits the
  code must hold.
- **Trust & data boundary** — when the change touches auth, credentials or
  secrets, outbound telemetry or data, untrusted input, or deployment, state
  what crosses the boundary and note whether a focused review (e.g.
  `security-review`) should run before delivery.
- **Exclusions** — nearby behavior left out.
- **What goes** — the files, symbols, tests and fixtures this change removes,
  and the behavior and data that must survive. Never plan repairs to them.
  Add earlier attempts you find while reading.
- **Passes** — a small change takes one. For larger changes, group related
  work into 2–4 substantial end-to-end passes, aiming for at most five.
  Start with the pass that could prove the plan wrong. Say exactly what it
  will show and sketch the rest. Each pass removes what it replaces.

"Done when" names behavior people see, not a list of files. Keep each finding
beside the decision it informs, with source paths. Quote what people said word
for word; the plan describes the work and gives its reader no orders.

In a Wave, derive "Done when" from its `GOAL.md` and check its `MEMORY.md`
for decisions already made.

## Principles

**Be bold.** When unsure, pick the more ambitious option.

**Be concrete.** "Fast" means nothing. "P95 under 100ms" means something.

**Land whole, build in slices.** A coherent change lands at once: splitting
it across PRs breeds adapters and dual states, and legacy/new splits,
fallbacks and dual writes need a reason and a deletion date. Inside the
Task, build it a slice at a time.

**Reshape before you add.** Map the concepts, types, writers and launch paths
that exist before adding another.
