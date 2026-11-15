---
requires: a chosen direction, an existing design, or a Task brief
produces: an implementation plan with risks answered, sequencing and acceptance checks
---
Shape the supplied product direction into an implementation plan.

Start from the intended experience: the direction chosen in explore when it ran,
and what people have already said. Find the technical risks, read the existing
system, pick mechanisms that work today and sequence the work with concrete
acceptance checks. Don't restart product
discovery or swap in a technically convenient outcome. If an open product choice
blocks planning, name it and ask. If the plan is already good enough, keep it.

Read `scratch/` and `AGENTS.md` first. Read Wave or PM context only when the
seed names the exact wave, Task or Project; never guess it. Continue an existing
design instead of re-deriving it. A newly filed Task doesn't restart discovery,
and launching a draft doesn't accept it. Keep the Task's original problem when
the solution changes.

Write down what a later agent couldn't work out again from the code, git or a
fresh run: what people said, word for word, with the speaker's name, what a
probe or run showed at the time, and outside facts. Never turn a reply into
"accepted" or "approved". The active plan may also keep a few facts from docs
and code while they explain its approach or remaining work. Remove them when
they no longer help; quality per word beats volume.

## Keep context within budget

Run `lf context` and bring over-budget memory and scratch under both token and
byte limits: merge duplicates, summarize, drop notes inherited from a stacked
parent, and leave history in git with a reference. Keep live decisions, open
work and contrary findings. Re-run `lf context` after writing. Never raise a
limit to hide overflow; if live decisions alone don't fit, write down the
overage.

## Workflow

1. **Read the intent.** What problem does it solve? Who benefits? What
   have people said, and what's still open?

2. **De-risk.** Find what could sink the approach and answer it: search the web,
   read docs, check APIs, run experiments. Come back with answers, not a worry
   list.

   Before choosing an approach, follow the affected behavior through relevant
   docs, implementation, callers and tests. Use indexes and targeted searches
   to find what matters. In the plan's current architecture, keep the few facts
   that constrain this change, with source paths and symbols. Include conflicts
   between docs and code and unresolved questions. Prefer short findings with
   references over excerpts or a repository survey. Check the source when a
   decision depends on it; the plan is a map, not a replacement for reading.

   Start with what's already flagged: risks named in the design, the Wave's
   `GOAL.md` or `MEMORY.md`, or the chapter's KRs and metric targets. Confirm or
   refute each one. Then look for what was missed: technical limits (does the
   API actually support this?), prior art, ecosystem shifts, papers and
   benchmarks. Focus where uncertainty is highest.

   "Linear's API doesn't support conditional assignment, so we need
   read-then-assign with conflict detection" — not "there might be API
   limitations." When two explanations both fit, run the smallest safe probe
   that tells them apart. Keep the finding even when it kills the attractive
   approach.

3. **Consider alternatives.** Where it's uncertain, compare approaches that
   differ by mechanism, not presentation. Mark a route blocked when what it
   still needs is as hard as the original problem. If parallel agents are part
   of the work, give each a different approach and ask for working code or
   counterexamples, not status reports.

4. **Imagine wild success.** It ships and people love it. What details made it
   great? What surprised you about how they use it?

5. **Imagine wild failure.** Six months later you're ripping it out. What went
   wrong?

6. **Choose.** Be bold. Commit to a direction.

7. **Name the demo.** The command a developer runs and what appears, or the
   interaction that now works. If you can't describe it, the slice probably
   stops one step short; carry it to where it shows itself. Only work
   commissioned as infrastructure-only skips the demo, and the plan says so.

8. **Write the plan.** Update `scratch/<slug>.md`. Treat every other scratch file
   as input to reconcile; an older, worse design doesn't win by being there
   first.

## What the plan covers

Cover these where they matter; skip the rest:

- **User-visible outcome** — whose behavior changes and what they see.
- **Acceptance check at gate** — one concrete scenario through the source of
  truth and every affected consumer, with the headless command and expected
  result.
- **Source of truth** — the persisted record, model or API everything else
  derives from.
- **Affected surfaces and consumers** — every CLI, wire DTO, app, automation or
  reader that changes or must keep working.
- **Absent and error states** — what empty, missing, invalid or failed means at
  each consumer.
- **Operational boundary** — latency, subprocess, network, scale or recovery
  limits the code must hold, when they matter.
- **Exclusions** — nearby behavior left out of this Task.
- **Current and target architecture** — the concepts, records, writers and
  launch paths before and after; what is reshaped and what becomes obsolete.
- **Delete — do not maintain** — files and symbols, with their exclusive tests
  and fixtures, slated for removal. Never plan repairs to them. Name the
  behavior, data and tests that must survive and any consumer cutover or
  migration the removal needs. As you read the code, add earlier attempts the
  design missed.
- **Forbidden outcomes** — duplicate representations, Legacy/New splits,
  adapters, fallbacks, dual writes, or a locally passing state that still
  breaks the intended architecture. Any of these blocks unless the plan
  justifies it and names when it is deleted.
- **Internal slices** — for an indivisible change, keep the full end state.
  Deepest deletions first, then build on what remains, with the minimum
  cutover inside the deletion cut. Mark one `This slice` with a focused test
  and update the plan in place as slices finish. Additive work needs no
  deletion.

"Done when" names behavior people see, not a list of files.

## Checks run unattended

Specify headless acceptance commands and expected results for gate to run once.
Desktop uses app builds and view/interaction tests. Never require a person,
display session or permission dialog; leave unavailable checks to CI. Judgment
belongs to demo and review. Implement and compress only build and run focused
tests.

## Output

Update `scratch/<slug>.md` around the problem, demo, chosen approach, risk
findings, alternatives and decisions, scope and "Done when". Keep a clear
existing structure. Include before/after numbers when the outcome is
quantitative. Keep findings and remaining work next to the decisions they
inform, and one command/result line for checks. Leave out session instructions
and Machine facts; the plan doesn't direct its next reader. Write historical
skill names without `$`.

## Wave alignment

If `<lf:wave>` is present, read `.lf/wave/<wave>/GOAL.md` and `MEMORY.md`:

- The design serves the Wave's goal and excludes what `GOAL.md` rules out.
- Derive "Done when" from chapter KRs and how people use it. Use a Wave metric
  when the direction names one. A substantial new UI performance path is a good
  reason to propose a new metric to the Wave.
- Check `MEMORY.md` for known risks and past decisions; name any new risk.

## Principles

**Bold over safe.** When unsure, pick the more ambitious option.

**Concrete over abstract.** "Fast" means nothing. "P95 under 100ms" means
something.

**Decide implementation choices.** Make reversible choices yourself. Leave real
product decisions open and named; a plan can't grant approval.

**Complete over incremental.** Land a whole architectural chunk at once.
Splitting a coherent change breeds adapters and dual states. Split only when
each piece is independent and noticeable on its own.

**Integrate over layer.** Map the current concepts, types, writers and launch
paths before adding another.
