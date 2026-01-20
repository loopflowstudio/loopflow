# Branch Names

"I want to have a more robust setup for branch names. Remote branch names should be timestamped so they're unique and maybe prefix our user name."

"Like jack.my-worktree-name.2026_01_23_45 or whatever"

"But locally the worktree name should just be my-worktree-name"

## What to build

A configurable branch naming schema that separates:
- **Worktree name** (short): user-provided, used for worktree directory path
- **Branch name** (full): schema-based, used for git branch (local and remote)

Example: User creates "my-feature"
- Worktree path: `../loopflow.my-feature`
- Git branch: `jack.my-feature.20260120_1234`

## Data structures

```python
# src/loopflow/lf/branch_names.py

@dataclass
class BranchNameConfig:
    """Configuration for branch name generation."""
    schema: str = "{name}"  # Default: no transformation
    # Supported placeholders:
    # - {name}: user-provided short name
    # - {user}: git config user.name or $USER (sanitized)
    # - {ts}: timestamp (YYYYMMDD_HHMM)
    # - {date}: date only (YYYYMMDD)


# In Config (config.py):
class Config(BaseModel):
    # ... existing fields ...
    branch_names: BranchNameConfig | None = None
```

Example configurations:

```yaml
# .lf/config.yaml

# Simple: just add timestamp for uniqueness
branch_names:
  schema: "{name}.{ts}"
  # "my-feature" → "my-feature.20260120_1234"

# Full: user prefix + name + timestamp
branch_names:
  schema: "{user}.{name}.{ts}"
  # "my-feature" → "jack.my-feature.20260120_1234"

# Date only (for daily uniqueness)
branch_names:
  schema: "{user}/{name}-{date}"
  # "my-feature" → "jack/my-feature-20260120"

# No transformation (default behavior)
branch_names:
  schema: "{name}"
  # "my-feature" → "my-feature"
```

### Changing your branch scheme

Edit `.lf/config.yaml`:

```yaml
# Before: no schema (branches = short names)
# branch_names not set

# After: add user prefix and timestamp
branch_names:
  schema: "{user}.{name}.{ts}"
```

**Existing worktrees are not affected.** The schema only applies to NEW worktrees created via `lfops wt create`. Old worktrees keep their original branch names.

**Team adoption:** Add to repo's `.lf/config.yaml` so everyone uses the same schema. Each user's `{user}` placeholder resolves to their own git username.

**Placeholders:**

| Placeholder | Example | Source |
|-------------|---------|--------|
| `{name}` | `my-feature` | User-provided short name |
| `{user}` | `jack` | `git config user.name` or `$USER`, sanitized |
| `{ts}` | `20260120_1234` | Timestamp: YYYYMMDD_HHMM |
| `{date}` | `20260120` | Date only: YYYYMMDD |

## Key functions

```python
# src/loopflow/lf/branch_names.py

def format_branch_name(short_name: str, config: BranchNameConfig | None) -> str:
    """Transform short name into full branch name using schema."""

def get_git_username() -> str:
    """Get username from git config user.name or $USER env var."""

def sanitize_for_branch(s: str) -> str:
    """Replace spaces, special chars with hyphens for valid git branch names."""
```

New worktree creation that separates short name from branch name:

```python
# src/loopflow/lf/worktrees.py

def create(
    repo_root: Path,
    short_name: str,
    base: str | None = None,
    branch_config: BranchNameConfig | None = None,
) -> Path:
    """Create a worktree with short name for path, schema-based branch name.

    Args:
        repo_root: Main repository path
        short_name: User-provided name (used for worktree directory)
        base: Base branch to create from
        branch_config: Optional branch naming schema

    Returns:
        Path to created worktree

    Worktree path: ../repo.short_name
    Branch name: schema-transformed (e.g., jack.short_name.20260120_1234)
    """
    branch_name = format_branch_name(short_name, branch_config)
    worktree_path = get_path(repo_root, short_name)  # Uses short name

    # Create worktree with explicit branch name
    subprocess.run([
        "git", "worktree", "add",
        "-b", branch_name,  # Full branch name
        str(worktree_path),  # Path uses short name
        base or "main",
    ], cwd=repo_root, check=True)

    return worktree_path
```

## CLI: `lfops wt create`

Need a new command since `wt switch --create` (worktrunk) doesn't support separating worktree name from branch name.

```python
# src/loopflow/lfops/wt.py

@wt_app.command("create")
def create_worktree(
    name: Annotated[str, typer.Argument(help="Short name for worktree")],
    base: Annotated[str | None, typer.Option("--base", "-b", help="Base branch")] = None,
):
    """Create worktree with schema-based branch name.

    The worktree directory uses the short NAME you provide.
    The git branch uses your configured schema (if any).

    Example:
        lfops wt create my-feature
        # Worktree: ../repo.my-feature
        # Branch: jack.my-feature.20260120_1234 (with schema)
    """
    repo_root = find_repo_root()
    config = load_config(repo_root)
    branch_config = config.branch_names if config else None

    path = worktrees.create(repo_root, name, base, branch_config)
    branch = get_current_branch(path)

    print(f"Created worktree: {path.name}")
    if branch != name:
        print(f"Branch: {branch}")
```

## UI changes

### Maestro changes

**WorktreeService.swift** - Use `lfops wt create` instead of `wt switch --create`:

```swift
func create(name: String, in repoURL: URL, baseBranch: String? = nil) async throws {
    var args = ["wt", "create", name]
    if let base = baseBranch {
        args.append(contentsOf: ["--base", base])
    }
    _ = try await runLfops(args, in: repoURL)
}
```

**Worktree sidebar** - Display both names when they differ:
- Primary: short name (from worktree path)
- Secondary/tooltip: full branch name

```swift
// Extract short name from worktree path
// ../loopflow.my-feature → "my-feature"
var shortName: String {
    path.split(separator: ".").last ?? branch
}
```

## Constraints

- Branch names must be valid git refs (no spaces, limited special chars)
- Timestamps use local time (simpler, matches user expectations)
- Schema applies to user-created worktrees only, not internal system worktrees
- Empty schema or `{name}` only = no transformation (backwards compatible)
- Username comes from `git config user.name` first, then `$USER` env var
- If username contains spaces/special chars, sanitize them
- Worktree path always uses the short name (no schema)

## Remote branch as key

The full branch name (schema-based) is the stable identifier for:
- **Worktree ↔ PR association**: `gh pr view {branch}` looks up by branch name
- **Pruneability detection**: if remote branch is deleted → worktree is prunable

### Branch lifecycle

```
1. lfops wt create my-feature
   → Creates worktree at ../repo.my-feature
   → Creates LOCAL branch: jack.my-feature.20260120_1234
   → NO remote branch yet

2. User works, commits locally...

3. lfops pr (or git push -u origin HEAD)
   → Creates REMOTE branch: origin/jack.my-feature.20260120_1234
   → Creates PR associated with that branch

4. PR merged, branch deleted on GitHub
   → Remote branch gone
   → Local branch still exists
   → Worktree is now prunable

5. lfops wt prune
   → Detects: remote branch deleted + PR not open = merged
   → Removes worktree and local branch
```

### Pruneability detection

Current logic in `worktrees.is_merged()` already handles this:

```python
# If PR existed but remote branch is gone → merged
if not _remote_branch_exists(repo_root, wt.branch) and pr_state != "open":
    return True
```

No changes needed - the branch name (full schema) is used consistently for:
- `gh pr view {branch}` - PR lookup
- `git show-ref --verify refs/remotes/origin/{branch}` - remote existence check
- `git merge-base --is-ancestor {branch} origin/main` - ancestry check

### What could break

**New branches without remote:** A freshly created worktree has no remote branch. Current code handles this:
- `_remote_branch_exists()` returns False
- But `pr_state` is None (no PR yet)
- So `pr_state != "open"` is True... but we also check other conditions

Actually, let me verify this doesn't cause false positives. The full check is:
1. `pr_state == "merged"` → prunable (correct)
2. `pr_state is not None` AND (`cherry_empty` OR `trees_match` OR `not remote_exists`) → prunable
3. Branch is ancestor of main → prunable

For a **new branch** (no PR, no remote):
- `pr_state` is None
- Condition 2 requires `pr_state is not None` → skipped
- Condition 3: new branch from main IS an ancestor initially

**Risk:** New branch created from main, before any commits, could be detected as "merged" because it's an ancestor of main.

**Fix:** Add check that branch has commits ahead of main before considering it merged:

```python
# In is_merged(), add early return for new branches
if wt.ahead_main == 0 and not wt.is_dirty:
    return False  # No work done yet, not merged
```

## Internal vs User worktrees

| Source | Short name | Branch name | Uses schema? |
|--------|------------|-------------|--------------|
| User via `lfops wt create` | "my-feature" | "jack.my-feature.20260120" | Yes |
| User via Maestro | "my-feature" | "jack.my-feature.20260120" | Yes |
| User via `wt switch --create` | "my-feature" | "my-feature" | No (worktrunk) |
| `lf pipeline` parallel | `_parallel-task-abc123` | `_parallel-task-abc123` | No |
| `lfd loop` iteration | N/A (cleanup after) | `goal/001` | No |

Internal worktrees (prefixed with `_`) bypass the schema.

## Done when

```bash
# 1. Configure schema
cat > .lf/config.yaml << 'EOF'
branch_names:
  schema: "{user}.{name}.{ts}"
EOF

# 2. Create worktree
lfops wt create my-feature

# 3. Verify worktree path uses short name
ls ../ | grep my-feature
# Output: loopflow.my-feature

# 4. Verify branch uses full name
cd ../loopflow.my-feature
git branch --show-current
# Output: jack.my-feature.20260120_1234

# 5. Push and create PR
lfops pr
# Should push branch, create PR

# 6. Verify PR lookup works
gh pr view
# Should show PR details

# 7. Simulate merge: delete remote branch
git push origin --delete $(git branch --show-current)

# 8. Verify pruneability detection
cd ../loopflow.main
lfops wt prune --dry-run
# Should list my-feature as prunable

# 9. Change schema and verify new worktrees use new schema
cat >> .lf/config.yaml << 'EOF'
# Changed to date-only
branch_names:
  schema: "{user}.{name}.{date}"
EOF
lfops wt create another-feature
git -C ../loopflow.another-feature branch --show-current
# Output: jack.another-feature.20260120 (no time, just date)

# 10. Verify internal worktrees unchanged
# (if running something that creates _parallel worktrees)
git worktree list | grep _parallel
# Should show: _parallel-implement-abc12345 (no schema)
```

**Maestro verification:**
1. Open repo in Maestro
2. Click "+" to create new worktree
3. Enter "test-feature"
4. Verify sidebar shows "test-feature" as primary name
5. Verify branch (hover/expand) shows full schema name
6. Create PR from Maestro
7. Verify PR appears and is linked correctly

## Open questions

1. ~~Should worktrunk (`wt` CLI) also support this schema?~~ No - `lfops wt create` is the loopflow-specific command. Users can still use `wt switch --create` for simple cases.

2. For collisions (same name + timestamp within same minute), should we add a random suffix automatically? Options:
   - Fail with "branch already exists"
   - Auto-append random suffix: `jack.my-feature.20260120_1234.a3f2`
   - Use seconds in timestamp: `YYYYMMDD_HHMMSS`

3. Should the schema support custom separators? Current examples use `.` but some might prefer `-` or `/`.
