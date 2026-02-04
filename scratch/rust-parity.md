# Rust LF Parity: Ops State + Testing/Rollout

Unified view of Rust `lf` parity work: current ops workflow status plus the testing and rollout plan to make Rust the primary CLI without UX drift.

## Current ops state (lf ops)

Rust `lf ops` is backed by the `loopflow-ops` crate, which orchestrates git, agents, lint, and PR workflows with progress callbacks. The CLI remains a thin wrapper, enabling reuse from `lfd`.

**Behavior**
- Auto-stage + auto-commit by default; `-m` remains optional.
- Lint runs before commit/PR/land when configured; failures can trigger a lint-fixer agent.
- Rebase conflicts trigger the rebase assistant; failures abort the workflow.
- Commit messages include flow lineage: `lf {flow_parents} {task}: {generated_title}`.

**Architecture**
```
rust/
├── loopflow-engine/     # Primitives (git, agent, prompt, config)
├── loopflow-ops/        # Workflows (commit/pr/land/next/abandon/rebase/lint)
├── lf/                  # CLI (thin wrapper)
└── lfd/                 # Daemon (can reuse loopflow-ops)
```

**In scope**
- `commit`, `pr`, `land`, `rebase`, `next`, `abandon` parity with Python
- Agent-backed commit/PR message generation
- Lint integration with fixer fallback
- PR lifecycle steps (draft/ready/auto-merge)

**Out of scope (deferred)**
- Wave-based branch naming/metadata updates
- Fish shell integration
- `lf ops doctor` and other non-blocking ops commands

## Testing + rollout plan

Goal: prove parity and transition Rust to primary without changing prompts, flows, directions, or artifact paths.

**Parity harness**
1. Prompt parity tests
   - Small, versioned fixtures under `tests/parity/fixtures/`.
   - Run Python and Rust with identical flags.
   - Normalize prompts (paths, timestamps, ordering) and compare byte-for-byte.

2. Golden prompt tests
   - YAML inputs + expected Markdown outputs.
   - Python generates goldens (source of truth) via `tests/goldens/update_goldens.py`.
   - Rust tests compare actual vs expected and write `.actual.md` on mismatch.

3. End-to-end workflows
   - Shell-based smoke tests for `lf ops` (full cycle + rebase conflicts).
   - Offline and deterministic; no external GitHub dependency.

**Rollout strategy**
- Phase B: Rust CLI primary with explicit fallback to Python for not-implemented paths.
- Phase C: PyPI ships Rust binaries; Python CLI becomes a thin wrapper.
- Phase D (optional): PyO3 bindings for Python API users.

**Done when**
- `uv run pytest tests/parity/test_prompt_parity.py` passes on all fixtures.
- `cargo test -p loopflow-engine golden_tests` passes with no `.actual.md` leftovers.
- `./tests/e2e/test_full_cycle.sh` and `./tests/e2e/test_rebase_conflict.sh` pass in CI.
- Rust CLI runs `lf run` and `lf ops` without fallback on the parity fixture set.

## Risks and bottlenecks
- `cargo run` in E2E scripts is slow/noisy; consider building once and reusing binaries.
- Fixture set is small; missing edge cases could hide prompt drift.
- Goldens are generated from Python; if Python behavior changes, goldens need regeneration.
