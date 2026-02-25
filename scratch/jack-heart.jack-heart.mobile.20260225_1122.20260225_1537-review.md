# Review: reverse branch naming for worktrees

## What was implemented

- Added reverse branch parsing in `engine/naming.rs`:
  - `BranchNameParts`
  - `parse_branch_name(branch, config)`
  - `wave_name(branch, config)`
- Added schema-aware worktree path resolution in `engine/worktrees.rs` via `worktree_path_with_config()` so branch-style input like `jack-heart.mobile.20260225_1122` resolves to `../<repo>.mobile`.
- Updated `create_with_schema()` to detect `origin/<input>` before minting a new branch:
  - if remote exists, check out that branch into the new worktree (tracking upstream when local branch is absent)
  - if remote does not exist, keep existing schema-based branch creation flow.
- Added tests for:
  - parsing default schema with/without `{words}`
  - parsing dotted wave names
  - custom schemas
  - extracting wave name
  - creating a worktree from an existing remote branch and verifying upstream tracking
  - worktree path extraction from branch-style input.
- Updated `docs/lfops.md` examples for `lf ops wt create/switch` and documented remote-branch checkout behavior.

## Key choices

- **Only trailing `{words}` is optional** in reverse parsing. This keeps stacked-branch support while preventing false positives from dotted non-branch strings.
- **Use configured schema when available** for worktree path extraction (`worktree_path_with_config`) so `{ts}` and separator choices stay aligned with repo config.
- **Remote branch detection happens first** in `create_with_schema()` to avoid creating accidental new branches when user passed an absolute branch name.
- **Track upstream only when needed** (`--track -b`) so existing local branches keep their current local identity.

## How it fits together

`lf ops wt create` now runs a branch-existence fork early: if `origin/<input>` resolves, it treats input as an existing branch and adds a worktree for that branch. Path naming is decoupled from raw input by passing the same input through `wave_name()` first, then falling back to filesystem sanitization. Reverse parsing lives entirely in `naming.rs` and is pure string/schema logic (no git dependency).

## Risks and bottlenecks

- Reverse parsing is schema-driven and best-effort; unexpected custom schemas can still fall back to sanitization behavior.
- `create_with_schema()` remote detection depends on local remote refs (`origin/*`) being available (i.e., fetch freshness).
- Full `cargo test --all` currently fails in this environment on two docker-specific tests due missing `/var/run/docker.sock`, not due this branch’s logic.

## What's not included

- No expansion of optional trailing parsing beyond `{words}`.
- No heuristic parsing outside schema patterns.
- No changes to Concerto wave-name validation rules.
- No changes to global docker test behavior/environment requirements.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all` *(fails only on docker-socket-dependent tests in this environment)*
- `cargo test -p loopflow parse_branch_name`
- `cargo test -p loopflow wave_name`
- `cargo test -p loopflow create_with_schema_uses_existing_remote_branch_and_wave_worktree_name`
