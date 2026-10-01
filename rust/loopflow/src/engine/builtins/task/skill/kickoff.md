---
requires: product direction, an existing design, or a Task brief
produces: an implementation plan with resolved risks, sequencing and acceptance checks
default_agent: claude
---
Shape the supplied product direction into an implementation plan.

Start from the intended experience and decisions already established. Investigate
technical risks, inspect the existing system, choose supported mechanisms and
sequence the work with concrete acceptance checks. Preserve accepted product decisions;
do not restart product discovery or substitute a technically convenient outcome.
If an unresolved product choice prevents planning, identify that choice for the
participant. Keep an already sufficient plan rather than forcing a rewrite.

## Task and design

Treat the Task as the user's problem and desired experience. Explore possible
solutions before choosing one; distinguish observations, proposed mechanisms,
real constraints, and accepted decisions. The design owns architecture,
implementation sequencing, and acceptance checks. A Task need not arrive with an
implementation plan. Preserve its original problem when the solution changes.

Continue an existing design and investigate its material gaps; a newly filed
Task does not require restarting discovery. Keep accepted decisions binding
and linked, preserve draft status and open questions, and honor the selected
Flow. Launching a draft does not approve it.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- Read wave/PM context only when the seed names the exact wave, task, project,
  or a concrete coordination question; never infer it or repair access as a
  prerequisite.
- Read the repo's agent doc (`AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Keep authored context within budget

Use the assembled `lf:context-budget` snapshot, or run `lf context --skill kickoff`
to read effective limits, their configuration sources, and current usage. Before
updating scratch or Wave memory, read complete sources named by excerpt pointers.
Bring over-budget material under both token and byte limits as part of this step.
Merge duplicates, summarize long evidence, remove obsolete notes inherited from a
stacked parent, and keep historical detail in git rather than ambient context.
Preserve live decisions, attribution, unresolved work, and contrary evidence;
keep a precise git reference when older detail still matters. Preserve uncommitted
evidence before removing it. Edit existing notes instead of accumulating reports.
Re-run the query after writing. Do not raise limits to conceal overflow. If the
live decisions alone cannot fit, record the concrete conflict and remaining overage.

## Workflow

1. **Recover the intent.** Read the design or Task brief. What problem does it solve? Who benefits? Which decisions are accepted, and what remains open?

2. **De-risk.** Before choosing implementation mechanisms, find the things that could invalidate your approach and resolve them. Search the web, read docs, check APIs, run experiments. The job isn't to list risks — it's to come back with answers.

   **Start with what's already flagged.** If the supplied design, wave `GOAL.md`, `MEMORY.md`, or current chapter KRs/metric targets call out specific risks, unknowns, or "what needs validation" — those are your first priority. Someone already thought these were dangerous enough to name. Research each one until you can confirm or refute it.

   **Then scan for what was missed.** Look across technical constraints (does the API actually support this?), prior art (have others tried and failed?), ecosystem shifts (will the ground move under us?), and domain knowledge (are there papers or benchmarks that constrain the solution space?). Not every dimension applies — focus where uncertainty is highest.

   The output of this step is concrete findings, not a worry list. "Linear's API doesn't support conditional assignment, so we need read-then-assign with conflict detection" — not "there might be API limitations." When two explanations remain plausible, run the smallest safe probe whose outcomes distinguish them. Preserve the observation even when it kills the attractive approach.

3. **Consider alternatives.** Where uncertainty warrants it, compare approaches that differ by mechanism,
   not presentation. Keep them independent long enough to expose their real
   strengths and exact gaps. Mark a route blocked when its remaining dependency
   is as hard as the original task; elegance does not make a deferred problem
   progress. If parallel agent work was explicitly authorized, assign approach
   families dynamically and require concrete artifacts or counterexamples, not
   status reports. Let the risks you found shape which alternatives are viable.

4. **Imagine wild success.** The feature ships and users love it. What details made it great? What surprised you about how people use it?

5. **Imagine wild failure.** Six months later, you're ripping it out. What went wrong? What did you miss?

6. **Make choices.** Given all this thinking, what's the right approach? Be bold. Commit to a direction.

7. **Name the demo.** Before writing the design, state the demo: the moment a
   developer sees the win working — the command they run and what appears, the
   interaction that now works. If you can't describe the demo, the slice is
   usually scoped one step short — carry it to where it shows itself. The one
   exception: work explicitly commissioned as infrastructure-only. Then say so
   in the doc instead of inventing a demo.

8. **Write the design.** Update `scratch/<slug>.md` with a concrete, actionable
   design. Treat every other scratch artifact as evidence to reconcile. It may
   contain an older or poor design; do not continue it merely because it was
   present first.

## Checks run unattended

Specify headless acceptance commands and expected results for gate to run once.
Desktop uses app builds and view/interaction tests. Never require a person,
display session or permission dialog. Defer unavailable checks to capable CI
without blocking earlier steps. Put judgment in demo/review. Implement/compress
only build and run focused tests; scratch keeps one command/result line.

## Computable design contract

Make the plan explicit where these boundaries matter; omit irrelevant sections:

- **User-visible outcome** — whose behavior changes and what they can observe
  when the Task holds.
- **Acceptance check at gate** — one concrete scenario that crosses the source of truth
  and every affected consumer, plus the headless command and expected result.
- **Source of truth** — the authoritative persisted record, model, or API and
  which views are derived from it.
- **Affected surfaces and consumers** — every CLI, wire DTO, app, automation,
  or downstream reader that must change or remain compatible.
- **Absent and error states** — what missing evidence, empty state, invalid
  input, or failed dependency means at each affected boundary.
- **Operational boundary** — when relevant, the latency, subprocess, network,
  scale, or recovery budget the implementation must hold.
- **Exclusions** — adjacent behavior deliberately left outside this Task.
- **Current and target architecture** — the concepts, authorities, persisted
  records, writers, and launch paths before and after the change; state what is
  reshaped and what becomes obsolete.
- **Delete — do not maintain** — carry forward or identify concrete files/symbols
  and their exclusive tests/fixtures slated for removal. Never plan repairs to
  them. Name required behavior, data, and tests that must survive, plus any
  consumer cutover or migration needed for removal.
- **Forbidden outcomes** — duplicate representations, Legacy/New splits,
  adapters, fallbacks, dual writes, or locally passing states that would still
  violate the intended architecture.
- **Internal slices** — for an indivisible change, keep the complete end state
  intact with its integration/deletion path and acceptance conditions. Order the deepest
  deletions first, then build on what remains. Include the minimum consumer
  cutover or migration in the deletion cut, without modernizing the old path
  first. Mark one `This slice` with a focused test, and update deletion targets,
  remaining work, and evidence in place rather than narrowing the plan.
  Additive work needs no invented deletion.

Done when names observable behavior, not a list of implementation artifacts.

## Output

Keep named, dated decisions, draft/accepted status and remaining work in the
plan. Record one check-result line. Omit session instructions and ambient Home
facts; the plan must not direct its next reader. Keep transcripts separate and
historical skill names unprefixed.

Update the existing plan at `scratch/<slug>.md`. Organize it around the problem,
demo, chosen approach, risk findings, alternatives and decisions, scope, and
Done when checks. Use the contract above for relevant boundaries; preserve a
clear existing structure rather than imposing a template. Include before/after
measures when the outcome is quantitative. Keep consequential findings and
remaining work beside the decisions they inform.

## Wave alignment

If `<lf:wave>` is present, check `wave/<wave>/GOAL.md` (and `MEMORY.md`) in docs:

- **Intent** — design must serve the wave's north star, stated in GOAL.md.
- **Evidence** — derive "Done when" from chapter KRs and user behavior. Use a
  Wave-owned signal when the direction names one. Leave room for feature work
  to reveal a better metric proposal; a substantial new UI performance path is
  a strong reason to capture one for Wave sponsorship.
- **Memory** — check `MEMORY.md` for known risks and prior decisions. If this design introduces a new risk, name it.
- Scope must exclude what GOAL.md marks as out of scope.

## Principles

**Bold over safe.** If you're not sure, pick the more ambitious option. Safe designs compound into mediocrity.

**Concrete over abstract.** "Fast" means nothing. "P95 latency under 100ms" means something.

**Resolve implementation choices.** Make supported reversible choices. Leave consequential product decisions explicit for the participant; a plan cannot manufacture approval.

**Complete over incremental.** Prefer landing an entire architectural chunk in one go. Splitting a coherent change into pieces creates backwards-compatibility adapters, dual states, and integration ambiguity. Only split when pieces are genuinely independent and each delivers something a user or developer would notice on its own.

**Sufficient for the next reader.** Keep consequential decisions, alternatives
and acceptance checks; omit repeated validation requirements and pass ledgers.

**Integrate over layer.** Map current concepts, types, authorities, writers, and
launch paths before adding another one. Name what the change reshapes and what
becomes obsolete. A Legacy/New split, v2, adapter, fallback, dual write, or
parallel store is blocking unless the design explicitly justifies and bounds
its deletion.
