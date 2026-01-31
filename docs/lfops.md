---
layout: default
title: lf ops Commands
---

# lf ops Commands

Operations and utilities. `lf ops` handles everything that's not launching a prompt.

## lf ops cp

Copy context to clipboard for use with web clients.

```bash
lf ops cp                     # copy repo docs
lf ops cp src tests           # copy specific paths
lf ops cp -e "*.pyc"          # exclude patterns
lf ops cp --no-lfdocs         # skip repo docs
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

## lf ops add

Create a new prompt file.

```bash
lf ops add my-task            # creates .claude/commands/my-task.md
lf ops add my-task -f         # overwrite if exists
```

---

## Git Workflow

## lf ops pr

Create or update a PR, open in browser.

```bash
lf ops pr
```

Generates a PR description based on the branch diff, creates the PR (or updates if one exists), and opens it in your browser.

Idempotent: run it to create, or again to update after more commits.

## lf ops land

Submit PR to merge queue.

```bash
lf ops land
```

Enables auto-merge on your PR. GitHub merges when CI passes and the merge queue clears. Run `lf ops wt prune` after merge completes to clean up.

## lf ops next

Land current PR and continue work on a new stacked branch.

```bash
lf ops next
```

Combines `lf ops land` + a branch switch in one command. After landing the current PR, it creates a new stacked branch from the current HEAD and switches the worktree to it. The worktree directory stays the same so you can keep working while the previous PR merges.

## lf ops commit

Generate commit message and commit.

```bash
lf ops commit
```

Stages changes, generates a commit message based on the diff, and commits.

Options:

| Flag | Description |
|------|-------------|
| `-p, --push` | Push after committing |

## lf ops doctor

Check dependencies.

```bash
lf ops doctor
```

Verifies that required tools are installed and working.

## lf ops version

Show loopflow version.

```bash
lf ops version
```

## lf ops summarize

Generate codebase summaries.

```bash
lf ops summarize src/           # Generate summary for src/
lf ops summarize -t 20000 src   # With specific token budget
lf ops summarize -a             # Regenerate all configured summaries
```

Creates pre-generated LLM summaries for large codebases. Summaries are cached and included in context when configured.

## lf ops rebase

Rebase current branch onto main.

```bash
lf ops rebase
```

Fetches main and rebases. If conflicts occur, launches an assistant to help resolve them.

## lf ops sync

Update local main to match origin.

```bash
lf ops sync
```

Fetches `origin/main` and updates your local main branch. Safe to run from any worktree.

## lf ops wt

Worktree helper commands.

### lf ops wt create

Create worktree with schema-based branch name.

```bash
lf ops wt create my-feature       # creates ../repo.my-feature
lf ops wt create my-feature -b develop   # from develop instead of main
lf ops wt create feature-B --stack       # stack on current branch
```

| Flag | Description |
|------|-------------|
| `-b, --base` | Base branch (default: main) |
| `-s, --stack` | Stack on current branch (branch from it, PR targets it) |

When using `--stack`, the new worktree branches from the current branch instead of main. PRs from stacked branches target their base branch while it's open, then retarget to main after the base PR merges.

### lf ops wt switch

Switch to a worktree by its short directory name.

```bash
lf ops wt switch my-feature       # switches to ../repo.my-feature
```

### lf ops wt list

List worktrees with prunable metadata.

```bash
lf ops wt list
```

### lf ops wt ci

Show CI status for the current branch.

```bash
lf ops wt ci
```

### lf ops wt prune

Remove worktrees whose branches have been merged.

```bash
lf ops wt prune           # interactive confirmation
lf ops wt prune --dry-run # show what would be pruned
lf ops wt prune --force   # skip confirmation
```

Finds worktrees where the PR was merged or the branch is an ancestor of `origin/main` (handles squash merges). Never prunes main/master or dirty worktrees.

| Flag | Description |
|------|-------------|
| `-n, --dry-run` | Show what would be pruned without removing |
| `-f, --force` | Skip confirmation prompt |

## lf ops abandon

Abandon a branch: close PR, remove worktree, delete branch.

```bash
lf ops abandon feature-branch
lf ops abandon feature-branch --force   # skip confirmation
```

| Flag | Description |
|------|-------------|
| `-f, --force` | Skip confirmation and force abandon with uncommitted changes |

## lf ops shell

Shell integration setup.

```bash
lf ops shell init       # print shell integration code
lf ops shell install    # install to shell config file
```

Adds the `wt` function for quick worktree switching: `wt my-feature` switches to `../repo.my-feature`.

## Typical Workflow

```bash
lf ops wt create my-feature       # create worktree (../repo.my-feature)
lf ops wt switch my-feature       # switch to worktree from another
# ... work on feature ...
lf ops commit                     # commit with generated message
lf ops pr                         # open PR (CI runs automatically)
# ... address review feedback ...
lf ops commit -p                  # commit and push
lf ops wt ci                      # check CI status
lf ops land                       # submit to merge queue
# ... wait for CI to pass and merge ...
lf ops wt prune                   # cleanup merged worktrees
```

## See Also

[`lf` reference](lf.md) · [Get Started](getting-started.md) · [Configuration](config.md)
