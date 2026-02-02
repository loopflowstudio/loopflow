# Design Review: Rust lf CLI & Roadmap Documentation

Branch: `jack-heart.rust.20260202_1034`

## What was implemented

1. **Rust `lf` CLI** (`rust/lf/`): Feature-complete port of the Python `lf` command, implementing:
   - Step execution with all flags (`-d`, `-a`, `-c`, `-m`, `--yolo`, `-i`, `-b`, `--web`, `--chrome`, `--wave`)
   - Flow execution via `tick_flow_with_runner()` with in-memory store
   - Inline prompts (`lf : "prompt"`)
   - Git operations (`lf ops rebase|push|land|pr|sync|next|commit|abandon`)
   - Context display (`lf context --tokens --trim`)
   - Config display (`lf config --global --repo`)
   - Discovery listings (`lf --list`, `lf flows`, `lf steps`, `lf directions`)

2. **Roadmap documentation** (`roadmap/rust/`): Complete 8-phase roadmap from CLI port through hosted deployment:
   - `01-lf-cli.md`: This branch's implementation
   - `02-lfd-primary.md`: Wire daemon to actually execute waves
   - `03-service.md`: launchd/systemd service integration
   - `04-distribution.md`: Homebrew, cargo install, curl installer
   - `05-auth.md`: WorkOS AuthKit integration for remote access
   - `06-executors.md`: Container/Kubernetes execution backends
   - `07-deployment.md`: Docker Compose and Helm chart packaging
   - `08-hosted.md`: Full SaaS control plane
   - `ARCHITECTURE.md`: Technical documentation of existing Rust components

3. **Python lfdocs refactor**: Simplified context gathering by consolidating `gather_docs`, `_gather_wave_roadmap`, and related functions into a single `gather_lfdocs()` function.

4. **Swift WaitingStateCard**: New SwiftUI component showing why a wave is blocked with actionable buttons (Review PRs, Collapse into One).

5. **Naming bugfix**: Fixed doubled-prefix issue in `parse_branch_base()` (e.g., `foo.bar.foo.bar.timestamp` → `foo.bar`).

## Key choices

| Decision | Rationale | Alternative rejected |
|----------|-----------|---------------------|
| External subcommand pattern | Step names are dynamic, not known at compile time. clap's `external_subcommand` routes unknown names to step execution. | Hardcoded step subcommands—inflexible |
| In-memory store for flow execution | CLI is stateless; daemon handles persistence. No socket needed for single flow run. | gRPC to lfd—adds latency and dependency for simple use case |
| `expect()` over `unwrap()` | Style guide requires reason strings outside tests. Improves debug context. | Bare `unwrap()`—no context on panic |
| macOS-only `pbcopy`/`open` | Phase 1 scope is macOS/Linux. Cross-platform abstraction deferred to Phase 4 distribution. | Abstract clipboard/URL handling now—premature |

## How it fits together

```
User runs: lf debug -c

          ┌─────────────────────────────────────────────────┐
          │ rust/lf/src/main.rs                             │
          │   clap parses args                              │
          │   routes to commands/step.rs                    │
          └─────────────────────────────────────────────────┘
                              │
                              ▼
          ┌─────────────────────────────────────────────────┐
          │ loopflow-engine                                 │
          │   gather_context() assembles prompt components  │
          │   format_prompt() builds final prompt           │
          │   launch_agent() spawns claude CLI              │
          └─────────────────────────────────────────────────┘
                              │
                              ▼
          ┌─────────────────────────────────────────────────┐
          │ claude --print -p <prompt>                      │
          │   execvp replaces process                       │
          │   agent takes over terminal                     │
          └─────────────────────────────────────────────────┘
```

For flows, the CLI creates an `InMemoryStore` and calls `tick_flow_with_runner()` in a loop until completion or interactive pause. This mirrors what lfd will do, but without daemon overhead.

## Risks and bottlenecks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Parity gaps with Python CLI | User confusion if behavior differs | `scratch/questions.md` documents known gaps; verification against Python test scenarios before 1.0 |
| `InMemoryStore` isn't production-ready | Fork state lost on crash during flow | Acceptable for CLI; daemon (Phase 2) provides persistent store |
| No Windows support | `pbcopy` and Unix socket assumptions | TCP fallback planned in Phase 4 distribution doc |
| Flow resumption unclear | Interactive step exits flow, no resume | Documented in questions.md; daemon handles resume in Phase 2 |

## What's not included

- **Daemon communication**: CLI is stateless. Wave management (`lf wave create`) and daemon features (`StreamOutput`, PTY connect) are Phase 2 scope.
- **Skill sources/external skills**: Listed as future enhancement in design doc.
- **Windows support**: Deferred to Phase 4.
- **Exact parity on `lf --list` output**: Simplified format; badges and sections from Python not replicated.
- **PR creation after flow**: `--pr` flag warns but doesn't execute. Documented in questions.md.

## Test results

```
cargo test -p lf                    # 2 passed
cargo test --workspace              # 37 passed (engine + daemon)
cargo clippy -p lf -- -D warnings   # clean
cargo fmt --check                   # clean
uv run pytest tests/                # 685 passed, 2 skipped
swift test --filter WaitingReason   # 4 passed
```
