# 06: Code Cleanup

**Finish line:** No cross-layer imports, no duplicate utilities, no dead code in match arms.

## Scope

**Layer violation.** Move `write_workspace_file`, `remove_workspace_file`, `cleanup_workspace_worktree` from `lfd::executor` to `engine`. `lf/commands/flow.rs` currently imports from the daemon layer for these.

**Duplicate utilities.** Remove one of the two `copy_to_clipboard` wrappers (one in `lf/commands/util.rs`, one in `lf/commands/ops/mod.rs`). Remove the duplicate `branch_exists` in `combine.rs` — use the engine's version.

**Dead code.** Remove or implement `sync` and `full` fields on `WtCommand::List`. Wire up `register_session`/`unregister_session` on `Scheduler` or remove. Remove `HttpState.output_hub` dead field.

**lfd binary parsing.** Migrate `bin/lfd.rs` management subcommands (`migrate`, `install`, `uninstall`, `start`, `stop`, `status`, `token`) from manual `args.get(1)` matching to Clap.

**Concerto cleanup.** Guard `ReplyDemoView` and `TerminalTestWindow` behind `#if DEBUG`. Remove or expose the `deepWine` palette. Fix `WaitingStateCard.extractOwnerRepo` to use async instead of blocking main thread.

**Python cleanup.** Bump `requires-python` to `>=3.10`. Collapse `wave_logs` error handling into `_raise_for_error`. Resolve the dual version source (`pyproject.toml` static + `hatch-vcs`).
