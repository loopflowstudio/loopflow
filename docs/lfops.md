---
layout: default
title: lfops Commands
---

# lfops Commands

Operations and utilities. `lfops` handles everything that's not launching a prompt.

## lfops cp

Copy context to clipboard for use with web clients.

```bash
lfops cp                     # copy repo docs
lfops cp src tests           # copy specific paths
lfops cp -e "*.pyc"          # exclude patterns
lfops cp --no-lfdocs         # skip repo docs
```

Options:

| Flag | Description |
|------|-------------|
| `-e, --exclude` | Exclude patterns |
| `-c, --clipboard` | Include current clipboard content |
| `--lfdocs / --no-lfdocs` | Include roadmap/, scratch/, and root .md files |
| `--diff / --no-diff` | Include raw branch diff |
| `--diff-files / --no-diff-files` | Include files touched by branch |
| `--summaries / --no-summaries` | Include pre-generated codebase summaries |

## lfops add

Create a new prompt file.

```bash
lfops add my-task            # creates .claude/commands/my-task.md
lfops add my-task -f         # overwrite if exists
```

---

## Git Workflow

## lfops pr

Create or update a PR, open in browser.

```bash
lfops pr
```

Generates a PR description based on the branch diff, creates the PR (or updates if one exists), and opens it in your browser.

Idempotent: run it to create, or again to update after more commits.

## lfops land

Submit PR to merge queue.

```bash
lfops land
```

Enables auto-merge on your PR. GitHub merges when CI passes and the merge queue clears. Run `lfops wt prune` after merge completes to clean up.

## lfops commit

Generate commit message and commit.

```bash
lfops commit
```

Stages changes, generates a commit message based on the diff, and commits.

Options:

| Flag | Description |
|------|-------------|
| `-p, --push` | Push after committing |

## lfops doctor

Check dependencies.

```bash
lfops doctor
```

Verifies that required tools are installed and working.

## lfops version

Show loopflow version.

```bash
lfops version
```

## lfops summarize

Generate codebase summaries.

```bash
lfops summarize src/           # Generate summary for src/
lfops summarize -t 20000 src   # With specific token budget
lfops summarize -a             # Regenerate all configured summaries
```

Creates pre-generated LLM summaries for large codebases. Summaries are cached and included in context when configured.

## lfops rebase

Rebase current branch onto main.

```bash
lfops rebase
```

Fetches main and rebases. If conflicts occur, launches an assistant to help resolve them.

## lfops sync

Update local main to match origin.

```bash
lfops sync
```

Fetches `origin/main` and updates your local main branch. Safe to run from any worktree.

## lfops wt

Worktree helper commands.

### lfops wt create

Create worktree with schema-based branch name.

```bash
lfops wt create my-feature       # creates ../repo.my-feature
lfops wt create my-feature -b develop   # from develop instead of main
lfops wt create feature-B --stack       # stack on current branch
```

| Flag | Description |
|------|-------------|
| `-b, --base` | Base branch (default: main) |
| `-s, --stack` | Stack on current branch (branch from it, PR targets it) |

When using `--stack`, the new worktree branches from the current branch instead of main. PRs from stacked branches target their base branch while it's open, then retarget to main after the base PR merges.

### lfops wt switch

Switch to a worktree by its short directory name.

```bash
lfops wt switch my-feature       # switches to ../repo.my-feature
```

### lfops wt list

List worktrees with prunable metadata.

```bash
lfops wt list
```

### lfops wt ci

Show CI status for the current branch.

```bash
lfops wt ci
```

### lfops wt prune

Remove worktrees whose branches have been merged.

```bash
lfops wt prune           # interactive confirmation
lfops wt prune --dry-run # show what would be pruned
lfops wt prune --force   # skip confirmation
```

Finds worktrees where the PR was merged or the branch is an ancestor of `origin/main` (handles squash merges). Never prunes main/master or dirty worktrees.

| Flag | Description |
|------|-------------|
| `-n, --dry-run` | Show what would be pruned without removing |
| `-f, --force` | Skip confirmation prompt |

## lfops abandon

Abandon a branch: close PR, remove worktree, delete branch.

```bash
lfops abandon feature-branch
lfops abandon feature-branch --force   # skip confirmation
```

| Flag | Description |
|------|-------------|
| `-f, --force` | Skip confirmation and force abandon with uncommitted changes |

## lfops shell

Shell integration setup.

```bash
lfops shell init       # print shell integration code
lfops shell install    # install to shell config file
```

Adds the `wt` function for quick worktree switching: `wt my-feature` switches to `../repo.my-feature`.

## Typical Workflow

```bash
lfops wt create my-feature       # create worktree (../repo.my-feature)
lfops wt switch my-feature       # switch to worktree from another
# ... work on feature ...
lfops commit                     # commit with generated message
lfops pr                         # open PR (CI runs automatically)
# ... address review feedback ...
lfops commit -p                  # commit and push
lfops wt ci                      # check CI status
lfops land                       # submit to merge queue
# ... wait for CI to pass and merge ...
lfops wt prune                   # cleanup merged worktrees
```

## See Also

[`lf` reference](lf.md) · [Get Started](getting-started.md) · [Configuration](config.md)
