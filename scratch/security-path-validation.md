# 02: Path Validation

## Problem

lfd accepts user-controlled strings that eventually touch the filesystem (IDs, repo/worktree paths, and upcoming file API paths). Today those checks are inconsistent and mostly “safe by convention” (UUID generation, path joining), not safe by boundary enforcement.

Who benefits: every self-hosted operator running lfd locally or remotely, plus Concerto/mobile clients that will depend on remote file endpoints.

Why now: Phase 08 (`/v0/waves/{id}/files`, `/v0/waves/{id}/file`) will expand filesystem reads. We need one hard guardrail before that lands.

## Approach

Adopt a single fail-closed path security layer in `lfd`, then route every filesystem entrypoint through it.

1. **Create shared path guards (`rust/loopflow/src/lfd/security.rs`)**
   - `path_within_root_existing(root, candidate)` for reads of existing paths.
   - `path_within_root_planned(root, candidate)` for writes/creates (canonicalize existing parent, then append final component).
   - Both reject: null bytes, absolute paths, `..`, Windows prefixes, and symlink escapes.

2. **Create shared ID/component validator**
   - `validate_safe_id(id)` for values used as path components (wave/run IDs and any future path-bearing IDs).
   - Allow only `[A-Za-z0-9_-]`, reject empty / `.` / `..` / separators / control chars.

3. **Wire all current filesystem surfaces**
   - `OutputHub`: resolve `<output_root>/<wave_run_id>.log` via `path_within_root_*` before read/write.
   - SQLite path config: validate configured DB path remains under `~/.lf`.
   - Worktree naming/path derivation: replace ad-hoc slash replacement with a strict fs-component sanitizer + validation.
   - Git hook payload paths: canonicalize and validate before any filesystem-derived follow-up logic.

4. **Phase 08 contract**
   - Require `path_within_root_*` in `/v0/waves/{id}/files` and `/v0/waves/{id}/file` before any `read_dir`, `metadata`, or file read.

5. **Test like an attacker**
   - Add unit tests for `..`, absolute paths, symlink escape, non-existent file under valid parent, null bytes.
   - Add route tests proving traversal attempts return `400`/`403` and never escape root.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep per-call inline checks (`starts_with`, `replace("../", "")`) | Fast to patch individual sites | Drifts over time; easy to miss one endpoint; weak against symlink/encoding edge cases |
| Sanitize-only strategy (normalize strings, then join) | Simple mental model | String sanitization alone misses filesystem reality (symlinks, prefixes, existing-parent behavior) |
| Capability/openat-style file descriptor sandbox for all file ops | Strongest OS-level isolation | Bigger architectural change; too large for Phase 02; keep for a future hardening phase |

## Key decisions

- We are enforcing wave security invariant **“No filesystem escape — user-controlled identifiers/paths cannot read or write outside declared roots.”**
- We are enforcing wave security invariant **“Fail closed on auth/trust ambiguity — when auth source or trust context is unclear, deny by default.”**

Decisions:

- **Centralize path validation** in one module; no new filesystem endpoint may bypass it.
- **Deny on ambiguity** (non-canonicalizable parent, invalid component, unknown root) instead of best-effort fallback.
- **Keep user-facing names flexible, constrain filesystem components**: display names can stay human; path components must pass strict validation.
- **Design for wild success**: when Phase 08 ships, file endpoints are already secure-by-default.
- **Design against wild failure**: avoid “one missed handler” regressions by making validation reusable + test-gated.

## Scope

- In scope:
  - Shared `path_within_root_*` + `validate_safe_id` utilities
  - Applying those checks to OutputHub, SQLite path resolution, worktree path derivation, and hook path handling
  - Documenting mandatory use in remote/08 file APIs
  - Security-focused tests for traversal and symlink escape

- Out of scope:
  - Container runtime isolation and cross-worktree mount restrictions (Phase 03)
  - Auth policy/provider routing decisions (Phase 06)
  - Rate limiting and payload-size controls (Phase 04)

## Done when

- Traversal payloads (`..`, absolute, symlink escape, null-byte) are rejected on all touched surfaces.
- No filesystem operation in touched codepaths can resolve outside its declared root.
- `wave/remote/08-api-expansion.md` explicitly requires `path_within_root_*` for file endpoints.
- Verification passes:
  - `cargo test -p loopflow path_within_root`
  - `cargo test -p loopflow validate_safe_id`
  - `cargo clippy -p loopflow -- -D warnings`
