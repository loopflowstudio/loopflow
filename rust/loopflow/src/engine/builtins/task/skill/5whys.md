---
requires: a failure and available evidence
produces: supported causal analysis and proposed prevention
---
Investigate why a failure happened and what would prevent it recurring.

Use the report and evidence already available, whether the failure is repaired
or still open. Keep recovery status explicit; analysis alone does not repair it.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- Read wave/PM context only when the seed names the exact wave, task, project,
  or a concrete coordination question; never infer it or repair access as a
  prerequisite.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Philosophy

**Follow causes beyond the first patch.** Ask what made the failure possible
and whether changing that condition would prevent it. Depth comes from evidence,
not a required number of answers.

**Investigate before you document.** Read the code. Run the commands. Trace the actual execution. The chain should reflect what you discovered, not what you guessed.

**"Human error" is never the root cause.** If someone made a mistake, ask: why was that mistake possible? What guardrail was missing? What feedback loop failed?

**Look for leverage at every level.** As you dig, ask: how could we change our prompts, processes, code, or tests to take a better course? The best fixes aren't patches—they're course corrections that make the failure mode impossible.

## Workflow

1. Understand the symptom thoroughly—read errors, logs, code
2. Ask "why did this happen?" and trace the answer (don't guess)
3. Take that answer and ask "why?" again
4. Follow the chain while evidence supports it; identify systemic causes when present
5. Look back: what questions did you skip? What branches unexplored?
6. For each level, ask: what change to prompts, process, migrations, code, or tests would have prevented this?

## Output

Update the existing incident analysis, or write a topic-named note under
`scratch/`. Preserve the working design and original observations. Include:

- What failed, who it affected, and the observed recovery status.
- The causal chain, with evidence supporting each link. Follow useful branches;
  do not invent a fifth cause or force every failure into a systemic explanation.
- Unanswered questions and evidence that contradicts the leading explanation.
- Proportionate prevention, why it would address the cause, and how to prove it.
  Distinguish proposed changes from fixes already made and accepted decisions.

Stop where the evidence stops. If no further change is worthwhile, say why.
Leave the analysis path and the useful next action in the summary so planning
can use the findings without treating every proposal as approved work.
