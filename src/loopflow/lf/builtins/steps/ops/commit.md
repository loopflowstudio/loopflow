---
produces: git commit
---
Generate a commit message and commit the staged changes.

## Goal

Create a clear, concise commit that captures what changed and why.

## Workflow

### 1. Check staged changes
```bash
git diff --cached
```

If nothing is staged, stage everything:
```bash
git add -A
```

### 2. Review the diff

Understand what changed:
- What files were modified?
- What's the nature of the change (feature, fix, refactor)?
- Is there a clear area/module the change focuses on?

### 3. Write the commit message

**Title style:**
- Lowercase, concise
- Optional area prefix when focused (e.g. `llm_http: add structured output`)
- Examples:
  - `llm_http: add structured output for pr messages`
  - `fix typo in readme`
  - `add dark mode toggle`

**Body style:**
- One sentence or a few bullets if needed
- Skip if the title is self-explanatory
- Explain "why" not "what"

### 4. Commit
```bash
git commit -m "title here" -m "body here if needed"
```

Or for title-only:
```bash
git commit -m "title here"
```

## Notes

- Do not ask questions. If anything is unclear, make the best assumption.
- Do not push. Just commit.
- If there are no changes to commit, say so and stop.
