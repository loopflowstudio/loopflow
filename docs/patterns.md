---
layout: page
title: Patterns
nav_order: 3
---

# Patterns

Workflows and recipes for common scenarios.

## Design-first development

Start with a design task that explores the problem before writing code.

```bash
lf wt create auth-feature
cd ../myrepo.auth-feature

lf design: add OAuth login    # interactive: discuss approach
lf implement                  # now implement what you designed
lf review
```

The design task can write notes to a file that implement reads:

```markdown
# .lf/design.lf

Explore how to implement:

{args}

Write a brief implementation plan to PLAN.md. Consider:
- What files need to change
- What dependencies are needed
- Edge cases to handle

Don't write code yet.
```

## Parallel features

Run multiple features simultaneously in separate worktrees:

```bash
# Terminal 1
lf wt create feature-a
cd ../myrepo.feature-a
lf ship -p &              # runs in background

# Terminal 2
lf wt create feature-b
cd ../myrepo.feature-b
lf ship -p &

# Check status from anywhere
lf status                 # shows running sessions
```

With maestro running, you'll get notifications when tasks complete:

```bash
lf maestro start          # start once, runs in background
```

## Model comparison

Race Claude and Codex on the same task:

```bash
lf implement --parallel claude,codex: add caching layer
```

This creates two worktrees (`implement-claude`, `implement-codex`) and runs both in parallel.

Compare results:

```bash
lf wt compare implement-claude implement-codex
```

## Batch pipeline

Run a full pipeline non-interactively:

```bash
lf ship -p                # -p for batch/print mode
```

In batch mode:
- No interactive prompts
- Auto-commits between tasks
- Pushes if `push: true` in config
- Opens PR if `pr: true`

## Review-only workflow

Just run review without implementation:

```bash
lf review                 # finds issues
lf review -p              # finds and fixes issues automatically
```

A review task might look like:

```markdown
# .lf/review.lf

Review the diff on the current branch against `main`.

Fix any issues you find. The deliverable is the fixes, not a written review.

Check for:
- Style guide violations (see STYLE.md)
- Bugs and edge cases
- Unnecessary complexity
- Missing tests for new code
```

## Context-heavy tasks

For tasks that need specific files:

```bash
lf implement -x src/models.py -x src/api.py: add user endpoints
```

Or set default context in config:

```yaml
context:
  - src/schema.py
  - src/types.ts
```

## Inline prompts

Quick one-off tasks without a task file:

```bash
lf : "fix the typo in the README"
lf : "add type hints to utils.py"
lf : "rename getUserById to findUserById everywhere"
```

## Custom pipelines

Define pipelines for different workflows:

```yaml
# .lf/config.yaml
pipelines:
  ship:
    tasks: [implement, review, test, commit]
    pr: true

  quick:
    tasks: [implement, commit]
    push: true

  polish:
    tasks: [review, test]
```

```bash
lf ship      # full workflow
lf quick     # fast iteration
lf polish    # cleanup pass
```

## Worktree cleanup

Remove worktrees for merged branches:

```bash
lf wt clean              # removes merged worktrees
lf wt list               # see what's left
```

## PR workflow

Open a PR from a worktree:

```bash
lf pr create             # opens PR on GitHub
```

Land a PR (squash-merge to main):

```bash
lf pr land               # merges and cleans up
lf pr land -a            # auto-merge when checks pass
```

## Commit message generation

The `commit` task can generate commit messages:

```markdown
# .lf/commit.lf

Review the staged changes and create a commit.

Write a clear commit message that explains WHY the change was made,
not just what changed. Follow conventional commits format if the
repo uses it.

Stage all changes and commit.
```

## Yolo mode

For trusted pipelines, skip all permission prompts:

```yaml
yolo: true
```

This is useful when:
- Running in a worktree (isolated from your main work)
- The pipeline is well-tested
- You want fully autonomous operation

Use with caution on untrusted code.
