---
name: ux-synthesize-concerto
description: Synthesize persona wave backlogs into a single ordered Concerto backlog.
loopflow: true
---
Synthesize persona wave backlogs into a single ordered Concerto backlog.

## Inputs

Read all items from these wave folders (if present):
- `wave/conductor/`
- `wave/improviser/`
- `wave/listener/`
- `wave/product-designer/`
- `wave/ceo/`

## Goal

Produce a single ordered set of backlog items in `wave/concerto/` that:
- Deduplicates overlapping issues across personas
- Preserves the most actionable framing
- Prioritizes by user impact and clarity

## Process

1. Cluster related items across waves (same problem, different framing).
2. Pick the strongest framing and concrete Build steps.
3. Assign an overall order (01, 02, 03...) for the Concerto backlog.
4. Write one file per item to `wave/concerto/`.

## Output format

Write files with:

- Filename: `<date>-<order>-<slug>.md`
- `date`: `YYYYMMDD`
- `order`: zero-padded sequence number
- `slug`: kebab-case title

Frontmatter:

```markdown
---
status: todo
phase: 1
persona: concerto
order: <order>
sources: [conductor, improviser, listener, product-designer, ceo]
---
```

Body:

```markdown
# <Title>

<One-line problem statement>

## Current
<What exists now>

## Problem
<Why it fails across personas>

## Build
<What to change>

## Done when
<Concrete success condition>
```

## Guidelines

- One issue per file
- Keep items right-sized (1–2 concrete improvements)
- Use the clearest wording from the source waves
- Prefer outcomes that help multiple personas at once
- Print an ordered list of filenames at the end
