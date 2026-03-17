# 04: Quality Signals

**Finish line:** The system detects shallow work, capability gaps, and human-system drift. These are the subtle signals — not failures, but degradation that compounds if unnoticed.

## Context

The easy blocks are mechanical: CI failed, merge conflict, wave stalled. The hard blocks are qualitative: the work is landing but it's not good enough. The human is approving but not reading. The agent is shipping code that compiles but hasn't been validated from a user's perspective.

These signals come from the tend flow's assess step, not from automated metrics. The chord uses judgment (informed by Letta memory) to detect patterns that numbers alone wouldn't catch.

## What to build

### Shallow work detection

The chord's assess step evaluates recent PRs against the work item's intent:
- Work item asked for a design decision → PR is mechanical code with no design rationale
- Work item asked for test coverage → PR adds code without tests
- Work item asked for user-facing polish → PR changes internals only
- Diff is small relative to the scope of the work item
- Multiple PRs on the same item, each making incremental progress without resolving it

Not a metric — a qualitative assessment by the tend flow agent, informed by the work item description, the diff, and Letta memory of what "good" looks like for this wave.

### Capability gap detection

The chord checks whether waves have what they need to validate quality:
- Does this wave run integration tests that touch the actual product?
- Does this wave's flow include validation against real user scenarios?
- Is there a way to see what the user sees (screenshots, app launch, API smoke test)?
- If not: surface as a block. "This wave is shipping code without validating the user experience."

### Human-system drift detection

The chord observes the human's engagement pattern:
- Time between block surfaced and human decision (trending up = disengaging)
- Depth of calibration responses (trending shorter = rubber-stamping)
- Frequency of human-initiated course corrections (trending to zero = auto-pilot)
- Number of tend cycles since human wrote trajectory notes

When the pattern suggests disengagement, surface as a block: "The system is producing work but you haven't engaged deeply in N cycles. Are you still connected to what's being built?"

## Done when

- Tend flow assess step evaluates PR quality against work item intent
- Capability gaps are identified and surfaced as blocks
- Human engagement patterns are tracked across calibration moments
- All three signal types surface in the block queue with useful context
- Signals are qualitative assessments, not mechanical metrics
