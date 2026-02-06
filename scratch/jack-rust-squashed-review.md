# jack-rust-squashed review

## What was implemented

Full Rust migration of loopflow. The Python CLI, daemon, proto, and all Python tests are removed. Replaced by five Rust crates:

- **lf** — CLI binary: step/flow execution, ops commands (commit, land, next, pr, rebase), discovery, output formatting
- **lfd** — daemon binary: HTTP API + WebSocket via axum, wave orchestration, scheduling, SQLite/Postgres store
- **loopflow-engine** — core library: context gathering, prompt assembly, flow parsing, agent invocation, config, git operations, naming, builtins, worktrees
- **loopflow-ops** — ops library: commit, PR, land, next, rebase, lint workflows with trace support
- **loopflow-py** — PyO3 bindings exposing engine + ops to Python (gradual migration bridge)

Additionally:
- **loopflow-test-support** — shared test infrastructure (TestRepo fixture with bare remote + working clone)
- **lfd HTTP decomposition** — `http.rs` split into `http/mod.rs`, `http/dto.rs`, `http/state.rs`, `http/routes/{waves,system,hooks,ws}.rs`
- **Command error helper** — `loopflow-engine::command::CommandError` centralizes shell command error reporting
- **Branch naming fix** — `format_branch_name` now uses the default schema when no config exists, preventing name collisions in `next` and worktree workflows
- **Release workflow** — GitHub Actions workflow for cross-platform binary builds and install script generation

## Key choices

1. **HTTP-only for lfd** — gRPC/tonic removed. axum + WebSocket covers the same use cases with simpler tooling. Auth/registration/credentials/machine_id modules removed.

2. **Builtins embedded via `include_str!`** — Step, flow, direction, and ops prompts compiled into the binary. No runtime file discovery needed for built-in content.

3. **DTO layer for HTTP responses** — Domain types (`crate::types::*`) mapped to DTOs via helper functions, keeping storage models decoupled from the API surface.

4. **Default branch naming** — When no config exists, `format_branch_name` applies the default schema (`{user}.{name}.{timestamp}.{words}`) instead of passing through the raw name. This prevents `next` from trying to create a branch that already exists.

5. **Shared test infrastructure** — `loopflow-test-support` provides `TestRepo` with bare remote, working clone, and helpers (create_file, commit, push, branch, checkout). Used across engine and ops tests.

## How it fits together

```
lf (CLI) ──→ loopflow-engine (context, prompt, flow, agent, naming)
                    │
lfd (daemon) ──→ loopflow-ops (commit, PR, land, next, rebase)
                    │
loopflow-py ──→ both (PyO3 bindings for Python callers)
```

The engine is stateless — it gathers context, formats prompts, and builds agent commands. The ops crate orchestrates git + GitHub CLI workflows. lfd wraps both in an HTTP server with scheduling and wave management. lf is the user-facing CLI.

## Risks and bottlenecks

- **Test coverage gap** — Rust has ~256 tests vs Python's ~10,700 lines. Core engine (config, context, prompt, flow, git, naming) is well-covered. Ops workflows now have integration tests. Agent launch, discovery, and daemon coverage are thinner.
- **Summaries not loaded** — `summaries: Vec::new()` with a TODO in prompt.rs. Not blocking but summaries won't appear in prompts until implemented.
- **Unused fields in GatherContextOpts** — `inline`, `step_args`, `area` are set by callers but not fully consumed. Scaffolding for features still being ported.
- **lfd output_hub** — `HttpState.output_hub` is declared but reserved for streaming endpoints (marked `#[allow(dead_code)]` with comment).

## What's not included

- Python CLI, Python tests, Python daemon — all removed
- Docker/docker-compose for lfd — removed (referenced gRPC setup)
- gRPC proto definitions and codegen — removed
- E2E shell scripts exist but aren't wired into `cargo test`

## Cleanup applied during gate

- **Fixed `format_branch_name` passthrough bug** — Without config, the function returned the raw short name unchanged. This caused `next_branch` to fail with "branch already exists" when the generated name collided with the current branch. Now applies the default schema.
- **Fixed rebase tests** — Tests didn't push main changes to origin before rebasing onto `origin/main`, causing rebases to trivially succeed instead of testing actual conflicts.
- **Fixed worktree test assertion** — Updated to match new default-schema behavior.
- **Added TestRepo helpers** — `push()` and `push_new_branch()` for tests that need remote tracking.
- **cargo fmt + clippy** — All clean, zero warnings.
