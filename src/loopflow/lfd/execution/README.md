# Execution

Runs flows as subprocesses. Manages worktrees for parallel execution.

## Flow Execution

```
Wave (persistent worktree @ branch-1)
│
├── Sequential steps run in-place
│
├── Parallel steps spawn temp worktrees, merge back
│
├── Fork spawns temp worktrees, runs in parallel
│   └── Synthesize reads diffs, writes to parent
│
├── PR created → main_branch
│
└── (Next iteration uses same worktree, new branch)
```

## Fork Execution

Fork runs multiple agents in parallel, each in its own temporary worktree. Synthesis combines their outputs into the parent worktree.

Example flow (from `builtins/flows/roadmap.py`):
```python
Fork(
    {"direction": "infra-engineer"},
    {"direction": "designer"},
    {"direction": "product-engineer"},
    step="roadmap",
    synthesize={"direction": "ceo"},
)
```

### Process Diagram

```
Wave (persistent worktree @ wave-main)
│
├─ Fork creates temp worktrees from HEAD
│   ├─ fork-flow-1/ (direction: infra-engineer)  ─┐
│   ├─ fork-flow-2/ (direction: designer)        ─┼─ concurrent.futures.ThreadPoolExecutor
│   └─ fork-flow-3/ (direction: product-engineer)─┘
│
├─ Each fork runs its step independently
│   └─ Commits stay in fork worktree
│
├─ Synthesize reads all fork diffs
│   └─ Writes unified result to parent worktree
│
├─ Cleanup: delete fork worktrees
│
└─ Flow continues (reduce, polish, etc.)
```

### Git Operations

| Phase | Operation | Command |
|-------|-----------|---------|
| Fork create | Create temp worktree | `git worktree add -b <branch> <path> <base>` |
| Fork create | Reset to parent HEAD | `git reset --hard <branch>` |
| Fork create | Clean untracked | `git clean -fd` |
| Fork run | Commit changes | `git commit -m "[fork-N] ..."` |
| Fork run | Get diff for synthesis | `git diff <base-commit>..HEAD` |
| Synthesize | Commit unified result | `git commit -m "..."` |
| Cleanup | Remove fork worktree | `wt remove <name>` (via worktrees.remove) |

### Worktree Types

**Persistent (wave's worktree):**
- Created once when wave starts
- Survives across iterations
- Located at `../repo.branch-name`
- After PR merges, `move_worktree()` switches to new branch

**Ephemeral (fork worktrees):**
- Created at Fork start
- Named: `fork-{flow}-{n}`
- Located at `../repo.fork-{flow}-{n}`
- Deleted after Synthesize completes

### Wave Lifecycle

```
lfd loop ship src/
│
├─ Wave created with persistent worktree
│   └─ git worktree add -b <wave-main> <path> origin/main
│
├─ Iteration 1
│   ├─ Create iteration worktree: <prefix>/001
│   ├─ Run flow steps (may include Fork)
│   ├─ Push: git push -u origin <branch>
│   ├─ PR: gh pr create --base <wave-main>
│   └─ Cleanup iteration worktree
│
├─ Iteration 2...N (same pattern)
│
├─ Land to main (if merge_mode=land)
│   ├─ Push: git push origin <wave-main>
│   ├─ PR: gh pr create --base main --head <wave-main>
│   └─ Auto-merge: gh pr merge --squash --auto
│
└─ After merge, move_worktree() → fresh branch
    ├─ git worktree remove --force <path>
    ├─ git worktree add -b <new-branch> <path> origin/main
    └─ git push -u origin <new-branch>
```

## Parallel Steps

Non-fork parallelism (from step dependencies) uses similar mechanics:

```
├─ Create temp worktree per parallel step
├─ Run steps concurrently
├─ Merge each branch back: git merge --no-edit <branch>
└─ Remove temp worktrees
```

## PR Operations

| Operation | Command |
|-----------|---------|
| Push branch | `git push -u origin <branch>` |
| Create PR | `gh pr create --title <t> --body <b> --base <base>` |
| Auto-merge iteration | `gh pr merge --squash --delete-branch` |
| Auto-merge to main | `gh pr merge --squash --auto` |
| Enable auto-merge | `gh pr merge <pr> --squash --auto --subject <title>` |
| Check PR state | `gh pr view <pr> --json state` |

## Key Functions

- `run_iteration()` — Main entry point, creates iteration worktree, runs flow
- `_run_fork_synthesize()` — Spawns fork worktrees, runs agents in parallel, synthesizes
- `_run_collector_step()` — Runs single step via collector subprocess
- `_cleanup_fork_worktrees()` — Removes all fork worktrees after synthesis
- `_create_pr_to_main_branch()` — Push and create PR targeting wave's main branch
- `_land_to_main()` — Create/update PR from wave-main to main, enable auto-merge
