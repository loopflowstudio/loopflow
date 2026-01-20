# Branch Names

Configurable branch naming schema that separates:
- **Worktree name** (short): user-provided, used for worktree directory path
- **Branch name** (full): schema-based, used for git branch (local and remote)

Example: User creates "my-feature"
- Worktree path: `../loopflow.my-feature`
- Git branch: `jack.my-feature.20260120_1234`

## Configuration

```yaml
# .lf/config.yaml

# Full: user prefix + name + timestamp
branch_names:
  schema: "{user}.{name}.{ts}"
  # "my-feature" → "jack.my-feature.20260120_1234"

# Simple: just add timestamp for uniqueness
branch_names:
  schema: "{name}.{ts}"
  # "my-feature" → "my-feature.20260120_1234"

# Date only (for daily uniqueness)
branch_names:
  schema: "{user}/{name}-{date}"
  # "my-feature" → "jack/my-feature-20260120"

# No transformation (default behavior)
branch_names:
  schema: "{name}"
  # "my-feature" → "my-feature"
```

**Placeholders:**

| Placeholder | Example | Source |
|-------------|---------|--------|
| `{name}` | `my-feature` | User-provided short name |
| `{user}` | `jack` | `git config user.name` or `$USER`, sanitized |
| `{ts}` | `20260120_1234` | Timestamp: YYYYMMDD_HHMM |
| `{date}` | `20260120` | Date only: YYYYMMDD |

**Existing worktrees are not affected.** The schema only applies to NEW worktrees created via `lfops wt create`.

## Usage

```bash
# Create worktree with schema-based branch name
lfops wt create my-feature
# Worktree: ../repo.my-feature
# Branch: jack.my-feature.20260120_1234 (with schema)

# Create from different base
lfops wt create bugfix --base release-1.0
```

## Implementation

### Python

**src/loopflow/lf/config.py** - Configuration model:
```python
class BranchNameConfig(BaseModel):
    schema_: str = Field(default="{name}", alias="schema")
```

**src/loopflow/lf/branch_names.py** - Core functions:
```python
def format_branch_name(short_name: str, config: BranchNameConfig | None) -> str:
    """Transform short name into full branch name using schema."""

def _get_git_username() -> str:
    """Get username from git config user.name or $USER env var."""

def _sanitize_for_branch(s: str) -> str:
    """Replace spaces, special chars with hyphens for valid git branch names."""
```

**src/loopflow/lf/worktrees.py** - New function (keeps existing `create()` unchanged):
```python
def create_with_schema(
    repo_root: Path,
    short_name: str,
    base: str | None = None,
    branch_config: BranchNameConfig | None = None,
) -> Path:
    """Create worktree with short name for path, schema-based branch name."""
```

**src/loopflow/lfops/wt.py** - CLI command:
```python
@wt_app.command("create")
def create_worktree(
    name: Annotated[str, typer.Argument(help="Short name for worktree")],
    base: Annotated[str | None, typer.Option("--base", "-b")] = None,
) -> None:
    """Create worktree with schema-based branch name."""
```

### Swift (Maestro)

**WorktreeService.swift** - Uses `lfops wt create`:
```swift
func create(name: String, in repoURL: URL, baseBranch: String? = nil) async throws {
    var args = ["wt", "create", name]
    if let base = baseBranch {
        args.append(contentsOf: ["--base", base])
    }
    _ = try await runLfops(args, in: repoURL)
}
```

**Worktree.swift** - Display helpers:
```swift
var shortName: String {
    // Extract from path: ../repo.short-name → short-name
    let dirname = URL(fileURLWithPath: path).lastPathComponent
    if let dotIndex = dirname.firstIndex(of: ".") {
        return String(dirname[dirname.index(after: dotIndex)...])
    }
    return branch
}

var displayName: String { shortName }
```

**WorktreeSidebar.swift** - Shows short name with branch tooltip:
```swift
Text(worktree.displayName)
    .help(worktree.branch != worktree.displayName ? "Branch: \(worktree.branch)" : "")
```

## Pruneability

The full branch name is the stable identifier for PR lookup and pruneability detection.

New branches (no commits, no PR) are not prunable:
```python
# In is_merged(), after PR state checks:
if wt.ahead_main == 0 and pr_state is None:
    return False  # New branch, not merged
```

## Decisions

1. **Separate function**: `create_with_schema()` vs modifying `create()` - keeps existing behavior unchanged for internal use.

2. **Collision handling**: Fail with "branch already exists" rather than auto-appending random suffix. Users can retry with a different name or wait a minute.

3. **Schema separators**: Users can put any valid git ref character in their schema. No special handling needed.
