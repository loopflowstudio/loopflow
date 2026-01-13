---
requires: changes to commit
produces: .lf/COMMIT
---
Draft a commit message for landing this branch.

You're writing for someone who sees this in `git log` or `git blame` and needs to understand intent without reading the diff. Outcome over process. Why over what. Searchable words someone would grep for.

## Input

```bash
git diff main...HEAD --stat
```

**If no diff:** Stop. Nothing to commit.

**If on main:** Stop. This prompt is for feature branches.

**If diff exists:** Read it and write a commit message to `.lf/COMMIT`.

## Commit message format

```
<area>: <summary>

<body explaining what and why>
```

**Title:** Lowercase, under 50 chars. Optional area prefix for focused changes.

**Body:** One paragraph explaining what this branch accomplished and why. Skip if the title is self-explanatory.

## Style examples

```
maestro: add session tracking daemon

Sessions now write to SQLite and can be viewed via lf ops status.
The maestro daemon provides a web UI for tailing logs across worktrees.
```

```
fix worktree cleanup on branch delete
```

```
prompts: improve expand and iterate focus

Both prompts now direct agents to focus on code touched by the current
branch rather than generic improvements across the codebase.
```

## Common mistakes

- **Describing the diff** — The diff shows what changed. Explain why it matters.
- **Listing commits** — Summarize the branch's outcome, not its history.
- **Process language** — "Refactored X to use Y" → "X now does Y"
- **Co-author footers** — Don't add "Generated with Claude" or similar.

## Output

Write the commit message to `.lf/COMMIT`. Tell the user to review it and run `lf ops land` when ready.

