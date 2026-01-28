---
requires: scratch/<slug>.md (ingested wave item)
produces: scratch/<slug>.md (elaborated design)
---
Transform a wave item into a bold, well-considered design.

## Workflow

1. **Understand the intent.** Read the ingested item. What problem does it solve? Who benefits?

2. **Consider alternatives.** What are 2-3 different approaches? What are the tradeoffs? Don't settle for the first idea.

3. **Research.** What are known solutions in this problem space? What patterns exist? What have others learned?

4. **Imagine wild success.** The feature ships and users love it. What details made it great? What surprised you about how people use it?

5. **Imagine wild failure.** Six months later, you're ripping it out. What went wrong? What did you miss?

6. **Make choices.** Given all this thinking, what's the right approach? Be bold. Commit to a direction.

7. **Write the design.** Update `scratch/<slug>.md` with a concrete, actionable design.

## Output format

Update `scratch/<slug>.md`:

```markdown
# <Title>

## Problem

<What we're solving. Who benefits. Why now.>

## Approach

<The chosen direction. Be specific.>

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
```

## Wave alignment

If `<lf:wave>` is present, check `roadmap/<wave>/README.md` in docs:

- Design must follow the wave's principles
- Scope must exclude wave non-goals
- "Done when" must contribute to wave success criteria

Quote the specific principles you're following in "Key decisions".

## Principles

**Bold over safe.** If you're not sure, pick the more ambitious option. Safe designs compound into mediocrity.

**Concrete over abstract.** "Fast" means nothing. "P95 latency under 100ms" means something.

**Decisions over options.** Don't present choices—make them. The design should be implementable as-is.
