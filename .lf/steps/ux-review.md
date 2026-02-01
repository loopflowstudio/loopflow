---
requires: screenshot(s) in area
produces: roadmap/concerto/<date>-<direction>-<order>-<slug>.md
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
3. For each question that the UI fails to answer well, write a backlog item file

## Output format

For each issue found, write a backlog item file in `roadmap/<wave>/`:

- Use `--wave` if provided; otherwise infer from the worktree name.
- If no wave can be determined, default to `roadmap/concerto/`.
- Filename: `<date>-<direction>-<order>-<slug>.md`
  - `date`: `YYYYMMDD`
  - `direction`: the persona name
  - `order`: zero-padded sequence number for this run (01, 02, 03...) in the order found
  - `slug`: kebab-case title
- Keep one issue per file
- Use the format below
- After writing files, print an ordered list of filenames with their wave folder

```markdown
---
status: todo
phase: 1
persona: <direction-name>
screenshot: <screenshot-path>
order: <order>
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
- One issue per backlog item file
- Keep items right-sized: 1–2 concrete improvements, not a grab bag
- Order matters: use the sequence from this run so ingestion preserves priority
- Skip questions the UI already handles well
