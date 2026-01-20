# Prune & Sync: Worktree Maintenance Commands

## What to build

Two `lfops` commands for worktree maintenance:
1. `lfops prune` — removes worktrees whose branches have been merged into main
2. `lfops sync` — fetches origin and updates local main to match origin/main

Maestro uses the same underlying APIs.

## Data structures

No new types. Uses existing `Worktree` dataclass which already has `pr_state` ("open", "merged", "closed", "draft").

## Key functions

### Python (`src/loopflow/lf/worktrees.py`)

```python
def is_merged(wt: Worktree, repo_root: Path) -> bool:
    """Check if worktree's branch has been merged to main.

    Returns True if:
    - pr_state is "merged", OR
    - branch commits are all ancestors of origin/main (squash-merged or rebased)
    """

def find_merged(repo_root: Path) -> list[Worktree]:
    """Return worktrees whose changes have been merged into main."""
```

### Python (`src/loopflow/lfops/prune.py`)

```python
def register_commands(app: typer.Typer) -> None:
    @app.command()
    def prune(
        dry_run: bool = typer.Option(False, "--dry-run", "-n", help="Show what would be pruned"),
        force: bool = typer.Option(False, "--force", "-f", help="Skip confirmation"),
    ) -> None:
        """Remove worktrees whose changes have been merged into main."""
```

### Python (`src/loopflow/lfops/sync.py`)

```python
def register_commands(app: typer.Typer) -> None:
    @app.command()
    def sync() -> None:
        """Fetch origin and update local main to match origin/main."""
```

Uses existing `_helpers.sync_main_repo()` which handles both checked-out and non-checked-out cases.

### Swift (`Maestro/Maestro/Services/WorktreeService.swift`)

```swift
func prune(in repoURL: URL, dryRun: Bool = false) async throws -> [String]
// Returns list of pruned branch names (or would-be-pruned if dry run)

func sync(in repoURL: URL) async throws
// Calls lfops sync
```

### Swift (`Maestro/Maestro/AppState.swift`)

```swift
func pruneWorktrees(dryRun: Bool = false) async throws -> [String]

func syncMain() async throws
```

## UI changes

No manual prune button in Maestro. The API methods (`pruneWorktrees`, `syncMain`) are available in AppState for background/automatic pruning controlled by a settings toggle (handled in a separate branch).

## Constraints

- **Must fetch origin/main before checking** — local main may be stale; `_helpers.sync_main_repo()` does this
- **Squash merges need ancestor check** — PR state alone isn't sufficient; check if branch tip is reachable from main
- **Never prune main/master** — filter out these branches explicitly
- **Never prune dirty worktrees** — check `is_dirty` before removing

## Done when

### Sync

```bash
# From a worktree (not on main)
lfops sync
# Output: Fetching origin...
# Output: Updated main to origin/main
```

### Prune

```bash
# Create test branch, "merge" it to main
git checkout -b test-prune
echo "test" > test.txt && git add . && git commit -m "test"
git checkout main && git merge test-prune
git checkout test-prune

# Prune should find it
lfops prune --dry-run
# Output: Would remove: test-prune

lfops prune
# Output: Removed: test-prune
```

### Maestro

API methods available for background pruning (no manual UI in this branch).
