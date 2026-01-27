# Worktree Base Branches

Branch from in-flight work without waiting for it to land.

## The problem

"This branch is too big already. I want to work on new stuff in a separate branch that has this change in it."

Currently, worktrees always branch from main. You either:
1. Wait for your big branch to land (blocks you)
2. Keep piling onto the big branch (makes it bigger)
3. Manually manage stacked branches with git (error-prone rebasing)

## What to build

Make stacking trivial:

```bash
# I'm in worktree A, branch is getting big
lfops wt create feature-B --stack    # branches from current branch
```

Or from anywhere:

```bash
lfops wt create feature-B --base jack.feature-A
```

## Current state

The `--base` flag exists but defaults to `main`:

```bash
lfops wt create my-feature --base develop  # works today
```

Waves already use this pattern—`{wave}.main` is a staging branch, iterations branch from it. Parallel forks branch from the current branch. The infrastructure exists.

## The rebase question

> "I want to minimize rebasing—this is risky"

Stacking creates a rebase dependency. We minimize it by:
- Tracking `base_branch` and `base_commit`
- `lfops wt sync` handles rebasing (including squash merge recovery)
- PR targeting auto-updates when base lands

No cascading rebase automation—`lfops wt sync` is explicit.

## CLI API

```bash
lfops wt create feature-B --stack
lfops wt create feature-B -s
```

`--stack` means "use current branch as base". Equivalent to `--base $(git branch --show-current)`.

This:
1. Creates worktree B branching from current branch's HEAD
2. Records `base_branch` and `base_commit` in metadata
3. PR for B targets base branch while it's open, main after it merges
4. If base changes, `lfops wt sync` handles rebasing

`--stack` is always explicit—never the default, even when run from a non-main worktree.

## Data structures

```python
@dataclass
class Worktree:
    path: Path
    branch: str
    base_branch: str | None   # "feature-A" - which branch we stacked on
    base_commit: str | None   # SHA when we branched - for squash merge recovery
    # ...
```

`base_branch` already exists. Add `base_commit` to handle squash merges.

## PR targeting

PRs target the base branch if it's still open, main if it's merged:

```python
def get_pr_target(base_branch: str | None) -> str:
    if not base_branch or base_branch == "main":
        return "main"

    # Check if base branch PR is merged
    result = subprocess.run(
        ["gh", "pr", "view", base_branch, "--json", "state"],
        capture_output=True, text=True,
    )
    if result.returncode == 0:
        state = json.loads(result.stdout).get("state")
        if state == "MERGED":
            return "main"

    return base_branch
```

Workflow:
1. `lfops pr` while base is open → targets base branch (clean diff)
2. Base branch lands
3. `lfops pr` again → detects merge, updates target to main

## Squash merge recovery

When base branch squash-merges, B's parent commits disappear. Need `base_commit` to transplant:

```bash
# B branched from A at commit c3
# A squash-merged to main as S
# B has commits c4, c5 on top of c3

git rebase --onto origin/main <base_commit> HEAD
# Takes c4, c5 and replays onto main, skipping c1-c3
```

`lfops wt sync` would handle this automatically.

## Key functions

```python
# src/loopflow/lfops/wt.py

@wt_app.command("create")
def create_worktree(
    name: Annotated[str, typer.Argument(help="Short name for worktree")],
    base: Annotated[str | None, typer.Option("--base", "-b", help="Base branch")] = None,
    stack: Annotated[bool, typer.Option("--stack", "-s", help="Stack on current branch")] = False,
) -> None:
    if stack:
        # Get current branch
        result = subprocess.run(
            ["git", "branch", "--show-current"],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0 or not result.stdout.strip():
            typer.echo("Error: not on a branch", err=True)
            raise typer.Exit(1)
        base = result.stdout.strip()

    # ... rest uses base as before
```

No changes needed to `create_with_schema()`—it already accepts `base` parameter.

## UI changes

CLI only for now. Concerto later.

`lfops wt list` shows stack:

```
NAME           BRANCH                    BASE         STATUS
feature-A      jack.feature-A.0126       main         3 ahead
feature-B      jack.feature-B.0126       feature-A    2 ahead
```

## Constraints

- Can only stack on local worktrees (no remote branches)
- `lfops wt prune` shouldn't prune B just because A landed
- Rebase via explicit `lfops wt sync`, not automatic

## Failure modes

| Scenario | What happens | Recovery |
|----------|--------------|----------|
| Base squash-merges | B's parent commits gone | `lfops wt sync` uses `--onto` with base_commit |
| Base rebases | B's parents moved | `lfops wt sync` rebases onto new base tip |
| Base gets more commits | B missing A's new work | Rebase onto base to catch up (optional) |
| Base abandoned | B depends on orphaned work | `lfops wt unstack` to absorb and target main |
| Middle of stack lands | A→B→C, B lands | C retargets to A (next `lfops pr`) |
| Base worktree pruned | Metadata gone | Fall back to main as target |

## Commands

| Command | Behavior |
|---------|----------|
| `lfops wt create --stack` | Branch from current, record base_branch + base_commit |
| `lfops wt list` | Show stack relationships |
| `lfops pr` | Target base_branch if open, main if merged |
| `lfops wt sync` | Rebase onto base (or main if base merged) |
| `lfops wt unstack` | Absorb base, clear metadata, target main |

## Done when

```bash
# Create stacked worktree
lfops wt create feature-B --stack
# → creates worktree branched from current
# → records base_branch and base_commit

# Stack another level
cd ../repo.feature-B
lfops wt create feature-C --stack

# List shows chain
lfops wt list
# feature-A    main        3 ahead
# feature-B    feature-A   2 ahead
# feature-C    feature-B   1 ahead

# PRs target immediate parent
cd ../repo.feature-C
lfops pr
# → gh pr create --base feature-B

# After B lands, C retargets
lfops pr
# → detects B merged, walks chain, targets main
```

## Multi-level stacks

Supports chains like `main → A → B → C`:

```
NAME        BRANCH              BASE        STATUS
feature-A   jack.A.0126         main        3 ahead
feature-B   jack.B.0126         feature-A   2 ahead
feature-C   jack.C.0126         feature-B   1 ahead
```

PR targeting walks the chain:

```python
def get_pr_target(worktree: Worktree) -> str:
    base = worktree.base_branch
    if not base or base == "main":
        return "main"

    base_wt = find_worktree_by_branch(base)
    if not base_wt:
        return "main"  # base pruned, fall back to main

    if is_pr_merged(base):
        return get_pr_target(base_wt)  # recurse up the chain

    return base
```

When A lands:
- B's PR retargets to main (next `lfops pr` call)
- C's PR retargets to B (still open)

When B lands:
- C's PR retargets to main

**Constraint:** Can only stack on local worktrees. Create the worktree first, then stack on it. No remote branch support.

## Scope

Lightweight stacking—enough to not need Graphite, without the complexity.

