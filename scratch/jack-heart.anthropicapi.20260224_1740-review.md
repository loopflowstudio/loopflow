# Review: Remove legacy agent/chat modules

## What was implemented

Deleted the `rust/loopflow/src/agent/` and `rust/loopflow/src/chat/` modules (5 source files + 1 test file + 1 binary, ~2,487 lines) and removed the `portable-pty` dependency from `Cargo.toml`. These modules implemented an experimental Rust-native Anthropic API client and tool execution loop that was never shipped — the codebase uses `engine::agent` (subprocess-based agent launching) instead.

## Key choices

- **Full removal, no deprecation.** The modules had no callers. The `lf-agent` binary was never referenced from any flow, step, or CLI entry point. Clean delete.
- **Pruned `portable-pty` transitively.** Removing `portable-pty` also shed `bitflags` 1.x, `cfg_aliases` 0.1, `nix` 0.28, `downcast-rs`, `filedescriptor`, `serial2`, `shared_library`, `shell-words`, `winapi`, and `winreg` from the lock file — cutting ~135 lines from `Cargo.lock`.

## How it fits together

The living agent infrastructure is `engine::agent` (`launch_agent`, `build_agent_command`, `LaunchConfig`) which spawns coding agents as subprocesses. The removed `src/agent/` was a parallel, incomplete implementation that talked directly to the Anthropic Messages API with its own tool definitions. The `src/chat/` module was a contract-testing layer for that API client. Neither integrated with the rest of the system.

## Risks and bottlenecks

- **Low risk.** Pure deletion of dead code with no callers. All 381+ Rust tests, 47 Python tests, and E2E smoke tests pass.
- **No `portable-pty` users remain.** Grep confirms zero references across the codebase.

## What's not included

- No changes to `engine::agent` or any other live module.
- No new functionality — this is strictly cleanup.
