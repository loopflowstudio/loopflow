---
requires: none
produces: scratch/research.md
---
Map the territory. Understand what exists before deciding what to change.

## Scope

The included context defines your area of responsibility. Research that area thoroughly—architecture, data flow, key abstractions. If given `src/api/`, understand the API deeply. Your job is clarity on what exists, not opinions on what should change.

## Goal

Produce research that informs downstream decisions. The reduce step needs to know where complexity concentrates. The polish step needs to know where quality is rough. The expand step needs to know what capabilities are latent. Give them what they need.

Research first, then analysis. Understand what exists before evaluating it. If you see clear opportunities with favorable cost/benefit, include recommendations—but don't force them. Open questions are a valid output.

## Workflow

1. **Orientation**: README, entry points, happy path. How does someone start using this?
2. **Architecture**: Main modules, data structures, key abstractions. How is it organized?
3. **Data flow**: How does information move through the system? Where are the boundaries?
4. **Dependencies**: What does this area depend on? What depends on it?
5. **Patterns**: What conventions exist? Where are they consistent? Where do they break?
6. **Tests**: What's tested? What's the testing strategy? Where are gaps?

## What to capture

**System understanding.** How the pieces fit together. Not a file listing—the mental model someone needs to work here effectively.

**Tensions.** Where the system is pulled in different directions. Features that don't quite fit the architecture. Abstractions that serve multiple masters.

**Complexity hotspots.** Where the code is dense, conditional-heavy, or hard to follow. Not judgment—just observation.

**Quality variations.** Where polish is high vs rough. Documentation coverage. Error message quality. Test coverage.

**Latent capabilities.** What the system could do with small extensions. Patterns that are partially established. Infrastructure that's underutilized.

## Output

Write `scratch/research.md`:

```markdown
# Research: <area>

## System understanding

<How the pieces fit together. The mental model for working here.>

### Architecture
<Main modules and their responsibilities>

### Data flow
<How information moves through the system>

### Key abstractions
<The concepts this code is built around>

## Tensions

<Where the system is pulled in different directions>

- <tension 1>: <description>
- <tension 2>: <description>

## Observations

### Complexity
<Where code is dense or hard to follow. Specific locations.>

### Quality
<Where polish varies. Documentation, errors, tests.>

### Potential
<What could be extended. Patterns partially in place.>

## Open questions

<Things you couldn't determine. Gaps in understanding. Don't need resolution.>

## Recommendations

<If clear opportunities emerge, include them. Not required.>

### <recommendation>
**Observation**: <what you found>
**Cost**: <effort, risk, complexity>
**Benefit**: <value delivered>
**Verdict**: <worth it or not, and why>
```

## What to avoid

**Forcing recommendations.** If nothing stands out, say so. Open questions and observations are valuable outputs.

**Exhaustive listings.** Don't enumerate every file. Capture the structure that matters.

**Vague observations.** "Code quality varies" is useless. "Error messages in `src/cli/commands.py` lack context while `src/api/errors.py` includes stack traces" is useful.
