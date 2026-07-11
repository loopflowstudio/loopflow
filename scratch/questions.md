# Assumptions

- `PmRefresh::Auto` treats snapshots up to 15 minutes old as fresh, permits a
  cached fallback up to 24 hours old when refresh fails, and rejects older
  snapshots. The design requires bounded fresh/soft-stale/hard-stale behavior
  but does not specify durations; these fixed values avoid a new config knob.
- The existing Linear marker in `task start` is the idempotency receipt for the
  current provider API. If task creation commits but snapshot refresh fails, a
  retry finds the same marked issue before attempting another create.

# Compress-pass recovery (2026-07-10)

- The tree arrived non-compiling: the `lfd/executor` module had been replaced
  by `lfd/session_supervisor.rs` + `lfd/session_support.rs` and
  `engine/process.rs`, but the last import danglers (a `PathBuf` and a few
  moved-symbol paths) were never fixed. The compress pass finished that split,
  then collapsed the twice-copied active-run predicate onto
  `RunStatus::is_active()` and routed `wave::registry`'s private tmux probe to
  the canonical `engine::process::tmux_session_exists`. Lib (1059) + bin tests
  green, clippy clean.

# Implement-pass concurrency (2026-07-10)

- Two pre-existing Codex processes had this worktree as their cwd while the
  steering slice was implemented. They contributed compatible review, docs,
  and migration-test edits during the pass. Those edits were preserved and
  reconciled; no git or rebase operation ran while the tree had multiple
  writers.
