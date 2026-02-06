# jack-rust-squashed review

## What was implemented

Full Rust port of loopflow. The Python CLI (`src/loopflow/`) is removed and replaced by Rust crates:

- **lf** (`rust/lf/`): CLI binary — step/flow execution, ops commands
- **lfd** (`rust/lfd/`): daemon binary — HTTP API + WebSocket, wave orchestration, scheduler
- **loopflow-engine** (`rust/loopflow-engine/`): core library — context gathering, prompt assembly, flow parsing, agent invocation, config, git operations, builtins
- **loopflow-ops** (`rust/loopflow-ops/`): ops library — commit, PR, land, next, rebase, lint workflows
- **loopflow-py** (`rust/loopflow-py/`): PyO3 bindings exposing engine + ops to Python (for gradual migration)

The daemon (`lfd`) was also simplified: gRPC/tonic removed in favor of HTTP-only via axum, auth/registration/credentials modules removed, and the `proto.rs` build step eliminated.

## Key choices

1. **HTTP-only for lfd** — gRPC added complexity (protobuf codegen, tonic dependency) without clear benefit for a local daemon. axum + WebSocket covers the same use cases with simpler tooling.

2. **Builtins embedded via `include_str!`** — Step, flow, direction, and ops prompts are compiled into the binary. No runtime file discovery needed for built-in content.

3. **PyO3 bridge** — Instead of a hard cutover, `loopflow-py` wraps the Rust engine and ops so Python tests and CLI can call Rust code. The `abi3-py38` feature and `build.rs` linker flag ensure broad compatibility.

4. **Golden prompt tests** — Test fixtures in `tests/parity/fixtures/` verify that the Rust engine produces identical prompt output to expectations, catching regressions in prompt assembly.

## How it fits together

```
lf (CLI) ──→ loopflow-engine (context, prompt, flow, agent)
                    │
lfd (daemon) ──→ loopflow-ops (commit, PR, land, next)
                    │
loopflow-py ──→ both (PyO3 bindings for Python callers)
```

The engine is stateless — it gathers context, formats prompts, and builds agent commands. The ops crate orchestrates git + GitHub CLI workflows. lfd wraps both in an HTTP server with scheduling and wave management. lf is the user-facing CLI.

## Risks and bottlenecks

- **Golden test fixtures** were missing on this branch (the `tests/parity/fixtures/` directories). Created during this gate pass. If fixture content drifts from expected golden output, tests fail with a clear diff.
- **Unused fields in `GatherContextOpts`** (`inline`, `step_args`, `area`) — set by callers but not consumed by the engine. These are scaffolding for features still being ported. Not blocking but worth tracking.
- **Summary loading** — `summaries: Vec::new()` with a TODO in prompt.rs. Summaries aren't populated yet.

## What's not included

- Python test suite (`uv run pytest tests/`) — the Python code was removed; those tests are gone
- Swift/Concerto tests — unrelated to this branch
- E2E smoke tests — the shell scripts were removed with the Python CLI
- lfd Dockerfile and docker-compose — removed with gRPC
- lfd README — removed (referenced Docker/gRPC setup that no longer exists)

## Cleanup applied during gate

- Deleted `rust/lfd/src/machine_id.rs` (dead code, orphaned by auth/registration removal)
- Removed empty `grpc` feature flag from `rust/lfd/Cargo.toml`
- Fixed `cargo fmt` ordering in `rust/lfd/src/main.rs`
- Created missing golden test fixtures in `tests/parity/fixtures/`
- Updated `rust/loopflow-engine/README.md` architecture listing and status section
