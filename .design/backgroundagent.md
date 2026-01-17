# Background Agent Branch Flow

## What to build

A git workflow where each background agent gets a persistent "personal main" branch that accumulates changes, with individual PRs per iteration and the option to squash-merge everything at once.

## Current vs Proposed

**Current**: Each iteration creates a fresh branch (`emoji/agent-name/001`), either auto-merged to main or individual PR. No accumulation.

**Proposed**:
```
origin/main
    ↑ rebase (lfops rebase)
security-checker-main              ← persistent, accumulates all iterations
    ↑ PR per iteration
security-checker-fix-auth-flow     ← named by agent after design
security-checker-add-rate-limiting
```

Iteration branches start with a temp name (`security-checker-abc123`), then the agent renames to something descriptive once it knows what it's building.

## Data structures

```python
class MergeMode(Enum):
    AUTO = "auto"      # PR per iteration, auto-merge to personal-main
    PR = "pr"          # PR per iteration, wait for approval
    SILENT = "silent"  # No PRs, direct merge to personal-main

@dataclass
class AgentSpec:
    name: str
    repo: Path
    pipeline: str
    # ... existing fields ...

    # NEW: assigned at registration, persisted in agent file
    personal_main: str | None = None  # e.g. "myagent-main" or "myagent-1-main"
    merge_mode: MergeMode = MergeMode.AUTO
```

Agent file gains fields:
```yaml
---
repo: ~/myproject
pipeline: ship
personal_main: myagent-main   # assigned by lfd new, or on first run
merge: auto                   # auto (default), pr, or silent
---
```

Merge modes:
- `auto`: Create PR per iteration, auto-merge to personal-main (default)
- `pr`: Create PR per iteration, wait for manual approval before merging
- `silent`: No PRs, merge directly to personal-main

## Key functions

```python
# loopflow/lfd/agents.py

def register_agent(name: str, repo: Path, ...) -> AgentSpec:
    """Create agent definition and allocate personal-main branch."""
    personal_main = _allocate_personal_main(repo, name)
    # Creates branch from origin/main if doesn't exist
    # Writes to ~/.lf/agents/{name}.md with personal_main field
    ...

def _allocate_personal_main(repo: Path, agent_name: str) -> str:
    """Return available branch name: agent-main, agent-1-main, etc."""
    candidate = f"{agent_name}-main"
    if not _branch_exists(repo, candidate):
        return candidate
    for i in range(1, 100):
        candidate = f"{agent_name}-{i}-main"
        if not _branch_exists(repo, candidate):
            return candidate
    raise ValueError("too many collisions")
```

```python
# loopflow/lfd/runner.py

def run_agent_iteration(agent: AgentSpec, run: AgentRun) -> None:
    """Execute one iteration, merge to personal-main, open PR."""
    # 1. Create iteration branch with temp name
    temp_branch = f"{agent.name}-{random_suffix()}"  # e.g. "security-checker-x7k2m"
    worktree = create_worktree(agent.repo, temp_branch, base=agent.personal_main)

    # 2. Run pipeline - agent can rename branch during execution
    #    (especially after design phase when it knows what it's building)
    ...

    # 3. After success: get current branch name (may have been renamed)
    final_branch = get_current_branch(worktree)
    create_iteration_pr(agent, run, final_branch)
```

```python
# loopflow/lfd/naming.py

def rename_branch(worktree: Path, new_name: str) -> None:
    """Rename current branch. Called by agent via lfops rename or git."""
    # git branch -m <new_name>
    # Also renames worktree directory to match
    ...
```

Agents rename via standard git (`git branch -m security-checker-fix-auth`) or we provide `lfops rename <new-name>` that handles worktree directory too.

```python
# loopflow/lfops.py

def rebase(worktree: Path | None = None) -> None:
    """Rebase personal-main onto origin/main.

    Usage: lfops rebase [-w worktree]

    If in a personal-main worktree, rebases that branch.
    If in an iteration worktree, rebases its base (personal-main).
    """
    # 1. git fetch origin main
    # 2. git rebase origin/main
    # 3. If conflict: abort rebase, print instructions, exit 1
    # 4. git push --force-with-lease origin {branch}
    ...

def land(squash: bool = False, ...) -> None:
    """Land changes to main.

    Without --squash: land the current iteration PR (existing behavior)
    With --squash: squash-merge entire personal-main to origin/main
    """
    if squash:
        # Generate PR body using lfops commit style (LLM summarizes full diff)
        message = generate_commit_message(diff_against_main)
        # Create PR from personal-main to main with all accumulated changes
        gh pr create --base main --head personal-main --body message
        ...
```

## Workflow

### Agent registration
```bash
lfd new security-checker --repo ~/myproject --pipeline review
# Creates ~/.lf/agents/security-checker.md
# Allocates security-checker-main branch
# Creates branch from origin/main
```

### Agent runs
```bash
lfd start security-checker
# Creates worktree with temp branch: security-checker-x7k2m
# Runs design task → agent learns what it's building
# Agent renames branch: git branch -m security-checker-fix-auth-bypass
# Runs remaining pipeline (implement, review, etc.)
# Opens PR: security-checker-fix-auth-bypass → security-checker-main
# (PR auto-merges or waits for approval based on config)
```

### Staying current
```bash
lfops rebase -w ../myproject.security-checker-main
# Rebases security-checker-main onto origin/main
# Force-pushes to keep remote in sync
```

### Landing accumulated work
```bash
# Option A: land individual iteration
lfops land  # lands current iteration PR to personal-main

# Option B: squash everything to main
lfops land --squash  # creates PR: personal-main → main, squashed
```

## Prompt changes

Agents need to know they should rename their branch. Add to design task (or inject into agent context):

```markdown
## Branch naming

You're on a temporary branch. Once you know what you're building,
rename it to something descriptive:

    git branch -m <agent-prefix>-<descriptive-name>

Example: If you're fixing an auth bypass, rename to `security-checker-fix-auth-bypass`.
Keep the agent prefix so branches stay organized.
```

This could be:
- Injected automatically when agent runs (loopflow adds to prompt)
- Part of the design.lf task template
- In the agent's prompt field in its definition file

## Constraints

- **Branch naming must be deterministic**: Given agent name, we can find its personal-main
- **Iteration branches are ephemeral**: Deleted after merging to personal-main
- **Personal-main is long-lived**: Survives across many iterations
- **Rebase is manual**: User runs `lfops rebase` when needed; agent doesn't auto-rebase

## Decisions

1. **Iteration PRs auto-merge to personal-main** by default. Configurable via `merge: pr` to require approval.

2. **PR per iteration** is the default (visibility into each change). Configurable via `merge: silent` to skip PRs and just merge directly.

3. **Iteration worktrees deleted** after PR merged to personal-main. Logs preserved in `~/.lf/logs/`.

4. **Rebase is user-initiated**: Run `lfops rebase` to rebase personal-main onto origin/main. If conflicts, rebase aborts and user resolves manually.

5. **Squash PR body**: Auto-generated via `lfops commit` style (LLM summarizes full diff against main).

## Done when

```bash
# Register agent, verify personal-main created
lfd new test-agent --repo . --pipeline review
git branch -a | grep test-agent-main

# Run iteration, verify temp branch created then renamed
lfd start test-agent
# Agent runs, renames branch to something descriptive
gh pr list  # shows PR like "test-agent-fix-something → test-agent-main"

# Rebase onto updated main
git checkout main && git pull
lfops rebase -w ../loopflow.test-agent-main
git log test-agent-main --oneline | head -1  # should be ahead of main

# Squash-land all accumulated work to main
lfops land --squash -w ../loopflow.test-agent-main
gh pr view  # shows PR: test-agent-main → main with auto-generated body
```
