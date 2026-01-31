---
requires: screenshot
produces: backlog items
interactive: true
---

Review the screenshot through the lens of the given direction (persona).

Apply each question from the direction. For each question:
1. Can the current UI answer it positively?
2. If not, what's the friction?
3. What would fix it?

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
