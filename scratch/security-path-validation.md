# 02: Path Validation — Done

Stop treating path safety as "safe by convention." Any user-controlled value that reaches the filesystem must stay inside an explicit root.

## What shipped

### Shared path security module

`rust/loopflow/src/lfd/security.rs` now centralizes filesystem validation:

- `path_within_root_existing(root, candidate)` for reads of existing paths
- `path_within_root_planned(root, candidate)` for planned writes/creates
- `validate_safe_id(id)` for path-component identifiers
- `sanitize_fs_component(value)` for safe filesystem name derivation

Guards reject:

- absolute paths
- `..` traversal
- null bytes and control chars
- Windows prefixes
- symlink escapes outside the declared root

### Output logs are root-bound

`OutputHub` resolves `<output_root>/<wave_run_id>.log` through shared guards before read/write. Unsafe IDs are rejected and do not create files.

### SQLite DB path is root-bound

`LFD_DB_PATH` is now resolved relative to `~/.lf` using `path_within_root_planned`. Absolute overrides are rejected.

### Worktree path components are sanitized

Worktree path derivation now uses `sanitize_fs_component` instead of ad-hoc slash replacement, with explicit safe-ID validation.

### Git hook repo paths are canonicalized

`POST /hooks/git` now requires an absolute path, canonicalizes it, and rejects non-directory or invalid paths before emitting worktree update events.

### Phase 08 contract documented

`wave/remote/08-api-expansion.md` now explicitly requires `path_within_root_existing`/`path_within_root_planned` before file reads, metadata lookups, or directory listing.

## Test coverage

Added security-focused tests for:

- traversal and absolute path rejection
- symlink escape rejection
- null-byte rejection
- non-existent file planning under valid parent
- safe ID validation/sanitization
- output log traversal rejection
- git hook path canonicalization and relative-path rejection

## Security boundary

This phase prevents:

- user-controlled IDs/paths from escaping declared roots
- filesystem reads/writes via traversal or symlink escape in covered surfaces

This phase does not prevent:

- host-level compromise
- container runtime isolation failures (Phase 03)
- auth-policy isolation decisions (Phase 06)
