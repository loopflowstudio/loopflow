Draft a commit message for landing this branch.

## Workflow

```bash
# 1. See commit history on this branch
git log main..HEAD --oneline

# 2. See all changes
git diff main...HEAD --stat
git diff main...HEAD
```

Read the changes and write a commit message to `.lf/COMMIT`.

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

## What NOT to do

- Don't list individual commits. Summarize the branch's overall change.
- Don't add "Generated with Claude" or co-author footers.
- Don't use imperative mood in the body ("Add X" → "X now works").

## Output

Write the commit message to `.lf/COMMIT`. Tell the user to review it and run `lf ops land` when ready.

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.

