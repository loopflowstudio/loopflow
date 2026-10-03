---
requires: none
produces: a sourced answer or research artifact for the requested decision
---
Map the territory. Understand what exists before deciding what to change.

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

## Scope

Follow the requested question and the participant's depth of interest. A narrow
codebase question needs a direct sourced answer, not an architecture survey.
Investigate more broadly when the decision depends on it. In conversation,
answer what was asked and follow new questions; headlessly, return a bounded
answer from available evidence. Do not implement or offer unsolicited critique.

## Workflow

Follow the paths that matter to the question:

1. **Orientation**: README, entry points, happy path. How does someone start using this?
2. **Architecture**: Main modules, data structures, key abstractions. How is it organized?
3. **Data flow**: How does information move through the system? Where are the boundaries?
4. **Dependencies**: What does this area depend on? What depends on it?
5. **Patterns**: What conventions exist? Where are they consistent? Where do they break?
6. **Tests**: What's tested? What's the testing strategy? Where are gaps?

## Evidence discipline

- Separate observations, interpretations, and open hypotheses. A source, code
  path, trace, or experiment supports an observation; resemblance does not.
- When uncertainty would change the conclusion, keep two or more materially
  different explanations alive and run the smallest safe check that separates
  them.
- Test the resulting system model against the complete relevant source set,
  especially evidence that predates or contradicts the favored explanation.
- Treat a contradiction as a reason to revise the representation or boundary,
  not merely patch the local claim.
- If the user explicitly authorizes parallel agent research, divide work by
  independent approach family, keep a registry of evidence and exact gaps, and
  require concrete findings rather than status reports. Do not broadcast the
  favored route until the independent passes have exposed their own gaps.

## Output

Answer here when the reader needs an explanation. For sustained research,
update its existing note or write a topic-named artifact under scratch/ when a
later reader needs it. Include the system model, sources, counterexamples,
unresolved questions and recommendations only where useful to the decision.
Explain how the relevant pieces fit, with concrete examples of tensions,
complexity, quality gaps or latent capabilities when they bear on the question.
Open questions are a valid result; do not force recommendations or a report shape.

When contributing independent research to another active writer, use the exact
assigned artifact or a distinct topic path and leave the canonical design to
its owner. Publish a complete artifact atomically; a path alone does not deliver
its contents to another checkout. Preserve source attribution and proof limits.
