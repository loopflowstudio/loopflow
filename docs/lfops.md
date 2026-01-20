---
layout: default
title: lfops Commands
---

# lfops Commands

Git workflow commands for shipping code.

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

Enables auto-merge on your PR. GitHub merges when CI passes and the merge queue clears. Run `wt remove <branch>` after merge completes to clean up.

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

## lfops init

Scaffold `.lf/config.yaml`.

```bash
lfops init
```

Creates a starter config file in your repo.

## lfops install

Install dependencies.

```bash
lfops install
```

Installs Claude Code, Codex CLI, Gemini CLI, and worktrunk.

## lfops doctor

Check dependencies.

```bash
lfops doctor
```

Verifies that required tools are installed and working.

## lfops status

Show running sessions.

```bash
lfops status
```

Lists any tasks currently running in auto mode.

## lfops summarize

Generate codebase summaries.

```bash
lfops summarize src/           # Generate summary for src/
lfops summarize -t 20000 src   # With specific token budget
lfops summarize -a             # Regenerate all configured summaries
```

Creates pre-generated LLM summaries for large codebases. Summaries are cached in `.lf/summaries/` and included in context when configured.

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

## lfops prune

Remove worktrees whose branches have been merged.

```bash
lfops prune           # interactive confirmation
lfops prune --dry-run # show what would be pruned
lfops prune --force   # skip confirmation
```

Finds worktrees where the PR was merged or the branch is an ancestor of `origin/main` (handles squash merges). Never prunes main/master or dirty worktrees.

Options:

| Flag | Description |
|------|-------------|
| `-n, --dry-run` | Show what would be pruned without removing |
| `-f, --force` | Skip confirmation prompt |

## Typical Workflow

```bash
wt switch --create my-feature    # create worktree
# ... work on feature ...
lfops commit                     # commit with generated message
lfops pr                         # open PR (CI runs automatically)
# ... address review feedback ...
lfops commit -p                  # commit and push
lfops land                       # submit to merge queue
# ... wait for CI to pass and merge ...
lfops prune                      # cleanup merged worktrees
```

## See Also

[`lf` reference](lf.md) · [Built-in Tasks](builtins.md) · [Configuration](config.md)
