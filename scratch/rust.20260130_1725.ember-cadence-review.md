# Design Review: Rust lf ops + engine bridge

## What was implemented
- Added a Rust `lf-engine` CLI and expanded `loopflow-engine` with prompt assembly, agent launch, runtime execution, and git workflow operations.
- Added PyO3 bindings for the Rust engine to expose prompt/runtime/git APIs to Python.
- Routed `lf ops` git helpers through Rust when `internal.use_rust` is enabled, with Python fallbacks.
- Added `lf --version` path that defers to Rust when enabled and available.
- Updated user docs to reference `lf ops` commands instead of the `lfops` binary.

## Key choices
- Shell to `lf-engine` for git and ops behavior (CLI JSON bridge) instead of reimplementing in Python.
- Rust token counting uses tiktoken with a fallback; prompt assembly mirrors existing context priorities.
- Keep Python fallbacks for git operations when Rust is unavailable or disabled.
- Remove the `lfops` console script in favor of `lf ops` while preserving module usage.

## How it fits together
- Python CLI uses `lf-engine` for version reporting and `lf ops` routes git operations through Rust via JSON subprocess calls when enabled.
- Rust `loopflow-engine` provides prompt assembly, flow runtime, agent launching, and git operations with a CLI and PyO3 bridge.

## Risks and bottlenecks
- `lf --version` now consults config; invalid configs could still block Rust version reporting (Python fallback remains).
- `lf ops` doc updates cover commands, but any external references to `lfops` may still be stale.
- Rust `lf-engine` JSON output must remain stable for Python parsing; errors must remain JSON on stderr.

## What's not included
- Full deprecation cleanup of `lfops` references across internal reports or Swift Concerto integration.
- Rust `lfd` daemon or protocol changes (Stage 3+).
- Summary loading and LOOPFLOW.md embedding in prompt assembly remain TODOs in Rust.
