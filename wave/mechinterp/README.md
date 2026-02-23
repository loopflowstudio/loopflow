# Mechanistic Interpretability × Loopflow

Anyone who hosts their own model can study it. What none of them have is the *workflow context* — they see API calls in isolation. Loopflow is the other half of the data: structured workflow metadata that turns raw model logs into labeled experiments.

## Goals

1. **Develop a point of view** on what mech interp applied to agent workflows means. Not a pitch — a research perspective.
2. **Build the annotation layer** that makes collaboration valuable. Every LLM call carries structured metadata about its workflow context.
3. **Position for collaboration** through the Anthropic Fellows Program (May/July 2026 cohorts) or direct engagement.

## Research Questions

Shaped by recent mech interp results. Each connects a published finding to a loopflow-specific investigation.

### Assistant Axis drift across multi-step flows (Jan 2026)

Assistant behavior maps to a single direction in activation space; models drift from it 7.3x faster during emotionally charged conversations. Does drift accumulate across a multi-step flow? Each step is a fresh API call, but artifacts carry forward. Does a gate step anchor the model back toward the axis, or does accumulated context pull it further away?

### Unfaithful reasoning in gate verdicts (Mar 2025)

Models sometimes plan ahead without saying so and fabricate explanations. When a gate step says SHIP, is the model's internal reasoning faithful to its stated rationale? When an implement step follows a design doc, does the model attend to the design's structure or pattern-match surface features? Loopflow has ground truth (tests pass or fail, reviewers agree or disagree) that could label these cases.

### Instruction conflict resolution in layered context (2025)

Models detect conflicts between system prompts and user inputs but don't reliably resolve them per the intended hierarchy. Loopflow assembles multiple instruction sources: area docs, style guides, directions, step prompts. How does the model's conflict-detection machinery handle layered, non-contradictory-but-nuanced instructions? Does it choose or synthesize?

### Step-type awareness and introspection (Oct 2025)

Larger models show limited but real ability to detect artificially injected concepts (~20% detection rate). Can the model detect what *kind* of step it's in? Does its internal state contain "I'm implementing from a design doc" vs. "I'm reviewing code for issues"? Does structured workflow context create richer self-models than unstructured conversations?

### Loopflow-specific SAE features (Feb 2025)

Anthropic trained SAEs on Sonnet 4.5 to check for dangerous features pre-deployment; the model recognizes evaluation contexts 58% of the time. Can SAEs detect loopflow-specific activation patterns? If so, that's both a research finding and a practical lever for studying structured-workflow behavior.

## Key People

- **Jack Lindsey** — leads "model psychiatry." Studies persona drift, the Assistant Axis, introspection. Most aligned with agent-behavior-across-workflows questions.
- **Emmanuel Ameisen** — leads circuit tracing methods, mentors Fellows. Most likely interested in new data sources for attribution graphs.
- **Chris Olah** — sets the research agenda. The conversation to have when the thinking is sharp and the instrument is built.

## What We're NOT Doing

- Not building analytics or dashboards
- Not generating synthetic experiments with open-weight models
- Not pitching anyone yet — building the thinking first
- Not treating this as a product feature — it's a research collaboration play that happens to require infrastructure

## Open Questions

- What's the right annotation schema? What metadata per API call would a mech interp researcher actually want?
- How do you key workflow metadata to Anthropic's internal request logs? Is there a request ID or correlation mechanism?
- Is the Fellows program the right entry point, or is direct engagement with Jack Lindsey / Emmanuel Ameisen better?
- What's the minimum viable experiment that demonstrates the value of workflow metadata + model internals?
- Does Anthropic have existing work on multi-turn / agent interpretability?
