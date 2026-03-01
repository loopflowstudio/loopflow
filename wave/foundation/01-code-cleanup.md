# 02: Code Cleanup

**Finish line:** No cross-layer imports, no duplicate utilities, no dead code in match arms.

## Scope

**Layer violation.** `write_workspace_file`, `remove_workspace_file`, `cleanup_workspace_worktree` live in `lfd::executor` but are imported by `lf/commands/flow.rs`. Move them to `engine`.

**Duplicate `branch_exists`.** `ops/combine.rs` defines its own `branch_exists` (lines 231–236) instead of using the engine's version in `engine/worktrees.rs`. Remove the duplicate.

**Dead code.** `sync` and `full` fields on `WtCommand::List` exist but aren't wired up. `register_session`/`unregister_session` on `Scheduler` are `#[allow(dead_code)]` with "reserved" comments. Remove or implement.

**lfd binary parsing.** `bin/lfd.rs` management subcommands (`migrate`, `install`, `uninstall`, `start`, `stop`, `status`, `token`) use manual `args.get(1)` matching. Migrate to Clap.

**Concerto cleanup.** `ReplyDemoView` and `TerminalTestWindow` are registered as windows without `#if DEBUG` guards. The `deepWine` palette exists in `ThemePreview.swift` — remove or expose properly.

**Python cleanup.** `requires-python` is `>=3.8` but ruff targets `py310`. Bump `requires-python` to `>=3.10`. Collapse `wave_logs` error handling into `_raise_for_error`.

**Config env override.** `output_log_retention_days` has no `LFD_OUTPUT_LOG_RETENTION_DAYS` env override — every other config field supports env overrides via `apply_env_overrides()`.

## Already shipped

**`copy_to_clipboard` duplicate** — resolved, single wrapper remains in `lf/commands/util.rs`.

**Config test isolation** — `load_config_or_default_returns_defaults` now uses `TempDir` isolation.

**Dual version source** — resolved, version is solely in `pyproject.toml`.

**`WaitingStateCard.extractOwnerRepo`** — synchronous `Process` call in a button action, acceptable for this use case.
