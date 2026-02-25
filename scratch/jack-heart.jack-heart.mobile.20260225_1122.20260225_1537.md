# wave_name: reverse branch naming for worktrees

## What to build

`wave_name()` — a function that reverses the branch naming schema to extract the `{name}` component from a branch name. Worktree creation uses this to always install at `<repo>.<wave_name>` instead of the full sanitized branch name.

## The problem

Default branch schema: `{user}.{name}.{timestamp}.{words}`

Today `lf ops wt create jack-heart.mobile.20260225_1122`:
- Treats input as a new feature name
- Sanitizes to `jack-heart-mobile-20260225_1122`
- Creates worktree at `../loopflow.jack-heart-mobile-20260225_1122`
- Creates a *new* branch on top of that

After:
- Recognizes input matches the schema (it's a "remote-style absolute" branch name)
- Extracts `{name}` = `mobile`
- Creates worktree at `../loopflow.mobile`
- Checks out the existing remote branch `origin/jack-heart.mobile.20260225_1122`

## Data structures

```rust
/// Parsed components of a branch name according to the naming schema.
#[derive(Debug, Clone, PartialEq)]
pub struct BranchNameParts {
    pub user: Option<String>,
    pub name: String,
    pub timestamp: Option<String>,
    pub words: Option<String>,
}
```

## Key functions

```rust
// naming.rs

/// Reverse-parse a branch name using the configured schema.
/// Returns None if the branch doesn't match the schema pattern.
pub fn parse_branch_name(
    branch: &str,
    config: Option<&BranchNameConfig>,
) -> Option<BranchNameParts>

/// Extract the wave name ({name} component) from a branch name.
/// Convenience wrapper around parse_branch_name.
pub fn wave_name(
    branch: &str,
    config: Option<&BranchNameConfig>,
) -> Option<String>
```

```rust
// worktrees.rs — changes to create_with_schema

/// Before creating a new branch, check if the input matches an existing remote branch.
/// If so, check it out and track the remote instead of creating fresh.
///
/// Worktree path always uses wave_name() when available:
///   ../loopflow.mobile  (not ../loopflow.jack-heart-mobile-20260225_1122)
```

## Parsing approach

The schema is a template like `{user}.{name}.{timestamp}`. To reverse it:

1. Build a regex from the schema by replacing each `{placeholder}` with a named capture group
2. Literal characters (`.`, `-`, `/`) between placeholders become literal regex matchers
3. Capture group patterns:
   - `{user}` → `[a-z0-9_-]+` (sanitized username)
   - `{name}` → `[a-z0-9._-]+` (the wave name, can contain dots)
   - `{timestamp}` or `{ts}` → `\d{8}_\d{4}` (YYYYMMdd_HHMM)
   - `{date}` → `\d{8}` (YYYYMMDD)
   - `{words}` → `[a-z]+-[a-z]+` (magical-musical pair)

For the default schema `{user}.{name}.{timestamp}.{words}`:
```
^(?P<user>[a-z0-9_-]+)\.(?P<name>[a-z0-9._-]+)\.(?P<timestamp>\d{8}_\d{4})\.(?P<words>[a-z]+-[a-z]+)$
```

Applied to `jack-heart.mobile.20260225_1122`:
- No `{words}` segment → doesn't match the 4-segment default schema
- But *does* match a 3-segment schema `{user}.{name}.{timestamp}`

This means: try matching with each optional trailing segment removed. The words segment is the most commonly absent (stacked branches drop it).

**User pattern heuristic**: `{user}` is a sanitized human name — typically `firstname-lastname` (e.g. `jack-heart`) or a single lowercase word. Recognizing `lowercase-lowercase` as likely a user name (not a wave name) disambiguates the first segment.

**Greedy matching caveat**: `{name}` can contain dots, so `jack-heart.mobile.feature.20260225_1122` could parse as `name=mobile.feature`. The anchors on `{timestamp}` and `{words}` patterns disambiguate — work backwards from the known-format segments.

## Worktree path changes

```rust
// worktrees.rs

pub fn worktree_path(repo: &Path, name: &str, branch_config: Option<&BranchNameConfig>) -> PathBuf {
    let repo_root = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());

    // Try to extract wave name from branch-style input
    let dir_name = wave_name(name, branch_config)
        .unwrap_or_else(|| sanitize_fs_component(name));

    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo");
    repo_root
        .parent()
        .unwrap_or(repo_root.as_path())
        .join(format!("{repo_name}.{dir_name}"))
}
```

## Remote branch detection in create_with_schema

```rust
// In create_with_schema, before creating a new branch:

// 1. Check if input matches a remote branch
let remote_branch = format!("origin/{short_name}");
let is_remote = rev_parse(repo, &remote_branch).is_ok();

if is_remote {
    // Check out existing remote branch (tracking it)
    // git worktree add <path> <remote_branch>
    // (no -b flag — uses existing branch)
} else {
    // Current behavior: create new branch with schema naming
}
```

## Constraints

- `wave_name()` must work without git (pure string parsing against schema)
- Parsing is best-effort — heuristics like "lowercase-lowercase is probably a user" help but aren't bulletproof. Return None and fall back to current sanitization when unsure.
- The `{name}` component is the canonical wave identity; everything else is metadata

## Wave name validation (Concerto)

Concerto should enforce simple wave names on input: `[a-z][a-z0-9-]*`

- No dots (dots are schema separators)
- No underscores (timestamps use underscores)
- No uppercase
- Must start with a letter
- Examples: `mobile`, `auth`, `dashboard-v2`

This makes the parser's job trivial — a clean wave name is always unambiguous in the branch schema. The CLI can be more permissive (users can pass whatever), but the canonical path through Concerto keeps things clean.

## Done when

```bash
# Existing remote branch → checks out tracking branch, worktree at loopflow.mobile
lf ops wt create jack-heart.mobile.20260225_1122
# Created worktree: ~/src/loopflow.mobile
# Branch: jack-heart.mobile.20260225_1122 (tracking origin)

# New feature → creates new branch, worktree still at wave name
lf ops wt create mobile
# Created worktree: ~/src/loopflow.mobile
# Branch: jack-heart.mobile.20260225_1537

# wave_name unit tests pass
cargo test -p loopflow wave_name
cargo test -p loopflow parse_branch_name
```
