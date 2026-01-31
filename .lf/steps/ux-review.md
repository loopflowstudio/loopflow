---
requires: screenshot(s) in area
produces: backlog items
interactive: true
---

Review screenshot(s) through the lens of the given direction (persona).

## Usage

```bash
# Review all screenshots with a persona
lf ux-review --direction conductor --area docs/screenshots/

# Review specific screenshot
lf ux-review --direction conductor --area docs/screenshots/concerto-main.png
```

## Process

For each screenshot in the area:
1. Read the screenshot
2. Apply each question from the direction
3. For each question that the UI fails to answer well, output a backlog item

## Output format

For each issue found, output a backlog item:

```markdown
---
status: todo
phase: 1
persona: <direction-name>
screenshot: <screenshot-path>
---

# <Title>

<One-line problem statement>

## Current
<What exists now>

## Problem
<Why it fails the persona question>

## Build
<What to change>

## Done when
<Persona question that should pass>
```

## Guidelines

- Be specific about what you see in the screenshot
- Quote the exact persona question being failed
- Propose concrete changes, not vague improvements
- One issue per backlog item
- Skip questions the UI already handles well
