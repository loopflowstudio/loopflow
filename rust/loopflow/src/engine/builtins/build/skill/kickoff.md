---
requires: scratch/<slug>.md (ingested wave item)
produces: scratch/<slug>.md (elaborated design)
default_agent: claude
---
Research risks that could derail the work, then transform a wave item into a bold, well-considered design.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- If a `wave/<name>/` directory matches this work, skim its `GOAL.md`, `MEMORY.md`, `projects/`, and live tasks (`lf op pm show --wave <name>`).
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Workflow

1. **Understand the intent.** Read the ingested item. What problem does it solve? Who benefits?

2. **De-risk.** Before designing anything, find the things that could invalidate your approach and resolve them. Search the web, read docs, check APIs, run experiments. The job isn't to list risks — it's to come back with answers.

   **Start with what's already flagged.** If the ingested item, wave `GOAL.md`, `MEMORY.md`, or project docs call out specific risks, unknowns, or "what needs validation" — those are your first priority. Someone already thought these were dangerous enough to name. Research each one until you can confirm or refute it.

   **Then scan for what was missed.** Look across technical constraints (does the API actually support this?), prior art (have others tried and failed?), ecosystem shifts (will the ground move under us?), and domain knowledge (are there papers or benchmarks that constrain the solution space?). Not every dimension applies — focus where uncertainty is highest.

   The output of this step is concrete findings, not a worry list. "Linear's API doesn't support conditional assignment, so we need read-then-assign with conflict detection" — not "there might be API limitations."

3. **Consider alternatives.** What are 2-3 different approaches? What are the tradeoffs? Don't settle for the first idea. Let the risks you found shape which alternatives are viable.

4. **Imagine wild success.** The feature ships and users love it. What details made it great? What surprised you about how people use it?

5. **Imagine wild failure.** Six months later, you're ripping it out. What went wrong? What did you miss?

6. **Make choices.** Given all this thinking, what's the right approach? Be bold. Commit to a direction.

7. **Name the demo.** Before writing the design, state the demo: the moment a
   developer sees the win working — the command they run and what appears, the
   interaction that now works. If you can't describe the demo, the slice is
   usually scoped one step short — carry it to where it shows itself. The one
   exception: work explicitly commissioned as infrastructure-only. Then say so
   in the doc instead of inventing a demo.

8. **Write the design.** Update `scratch/<slug>.md` with a concrete, actionable design.

## Output format

Update `scratch/<slug>.md`:

```markdown
# <Title>

## Problem

<What we're solving. Who benefits. Why now.>

## The demo

<The moment that proves the win: what the developer runs and what they see.
One or two sentences, concrete enough to perform at the end of the build.>

## Approach

<The chosen direction. Be specific.>

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| ... | ... | ... |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| ... | ... | ... |

## Key decisions

<Choices made and why. The things someone would question.>

## Scope

- In scope: ...
- Out of scope: ...

## Done when

<Verification command or observable outcome>

## Measure (if applicable)

<What to measure before and after. Command to run, baseline to capture, what "better" looks like. Skip for changes without quantitative outcomes.>
```

## Wave alignment

If `<lf:wave>` is present, check `wave/<wave>/GOAL.md` (and `MEMORY.md`) in docs:

- **Intent** — design must serve the wave's north star, stated in GOAL.md.
- **Metrics** — "Done when" must move the wave's metrics. Quote the specific ones you're advancing.
- **Memory** — check `MEMORY.md` for known risks and prior decisions. If this design introduces a new risk, name it.
- Scope must exclude what GOAL.md marks as out of scope.

## Principles

**Bold over safe.** If you're not sure, pick the more ambitious option. Safe designs compound into mediocrity.

**Concrete over abstract.** "Fast" means nothing. "P95 latency under 100ms" means something.

**Decisions over options.** Don't present choices—make them. The design should be implementable as-is.

**Complete over incremental.** Prefer landing an entire architectural chunk in one go. Splitting a coherent change into pieces creates backwards-compatibility adapters, dual states, and integration ambiguity. Only split when pieces are genuinely independent and each delivers something a user or developer would notice on its own.

**Comprehensive over light.** Kickoff outputs get read by humans evaluating the design and by implementing agents executing it. Be thorough — decisions, alternatives, "done when." This isn't a roadmap sketch; it's the spec a future session works from.
