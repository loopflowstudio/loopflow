# 05: Code Cleanup

**Finish line:** No cross-layer imports, no duplicate utilities, no dead code.

## Scope

**Layer violation.** `write_workspace_file`, `remove_workspace_file`, `cleanup_workspace_worktree` live in `lfd::executor` but are imported by `lf/commands/flow.rs`. Move them to `engine`.

**Duplicate `branch_exists`.** `ops/combine.rs` defines its own `branch_exists` (lines 231–236) instead of using the engine's version in `engine/worktrees.rs`. Remove the duplicate.

**lfd binary parsing.** `bin/lfd.rs` management subcommands (`migrate`, `install`, `uninstall`, `start`, `stop`, `status`, `token`) use manual `args.get(1)` matching. Migrate to Clap.

**Concerto cleanup.** `ReplyDemoView` and `TerminalTestWindow` are registered as windows without `#if DEBUG` guards. The `deepWine` palette exists in `ThemePreview.swift` — remove or expose properly.

**Python cleanup.** `requires-python` is `>=3.8` but ruff targets `py310`. Bump `requires-python` to `>=3.10`. Collapse `wave_logs` error handling into `_raise_for_error`.

**Config env override.** `output_log_retention_days` has no `LFD_OUTPUT_LOG_RETENTION_DAYS` env override — every other config field supports env overrides via `apply_env_overrides()`.

**Release tag test gap.** `tag_and_push_ref` with a non-HEAD `target_ref` (used by `release_run` when tagging a merged commit) is exercised by the full flow but not unit-tested in isolation.
