# Reviews

Consolidated review notes for recent Rust parity work.

## 2026-02-03: Rust ops parity polish

**What was implemented**
- Added the `loopflow-ops` crate and wired `lf ops` to use it for commit/pr/land/next/abandon/rebase workflows.
- Implemented agent-backed commit/PR message generation, lint integration, rebase recovery, and PR lifecycle updates.
- Added a README for the new crate and cleaned up workflow edge cases (local merge strategy, lint skip behavior, sync failure handling).

**Key choices**
- Keep `lf` as a thin wrapper that delegates to `loopflow-ops` so `lfd` can reuse the same workflows.
- Use `Progress` callbacks for all UX output/confirmations to keep CLI and daemon flows consistent.
- Skip lint entirely when no checker is configured, rather than launching a fixer with no signal.

**How it fits together**
`lf ops` routes to `loopflow-ops` workflows, which orchestrate `loopflow-engine` git/agent primitives and the GitHub CLI, while surfacing status via `Progress`.

**Risks and bottlenecks**
- GitHub CLI availability and behavior differences can block PR flows; errors surface but are not retried.
- Rebase recovery relies on agent output; if the agent fails, the operation aborts.
- Local merge strategy uses `git land` primitives; behavior should be revalidated against Python parity.

**Not included**
- Wave-based branch naming and metadata updates in `next`.
- Fish shell integration and ops doctor/add/cp/version commands.
- PR base detection beyond worktree metadata (still defaults to main when ambiguous).

## 2026-02-04: Rust prompt parity harness

**What was implemented**
- Added a Rust `lf-prompt` helper binary to emit formatted prompts for parity testing.
- Added Rust golden prompt tests plus Python/Rust prompt parity tests with fixtures and golden files.
- Added E2E shell scripts for `lf ops` full-cycle and rebase-conflict workflows.
- Added a parity/testing design note and updated `TESTING.md` with Rust/E2E commands.

**Key choices**
- Use a dedicated `lf-prompt` binary to avoid coupling parity tests to the main CLI and keep inputs explicit.
- Normalize prompts (paths, line endings) before comparison to keep goldens stable across environments.
- Keep E2E scripts minimal and repo-local by using temp git repos and `cargo run` instead of external tooling.

**How it fits together**
Python generates goldens and parity cases; Rust uses `lf-prompt` + `gather_context` to produce equivalent prompts.
The Python parity test compares Python vs Rust outputs, while Rust golden tests compare against expected prompt files.

**Risks and bottlenecks**
- `cargo run` in E2E scripts is slow and can be noisy in CI; consider building once and reusing binaries.
- Parity fixtures are small; missing edge cases could hide prompt drift.
- Goldens are generated from Python; if Python behavior changes, goldens need regeneration.

**Not included**
- No CI wiring for Rust/E2E tests yet.
- No additional parity fixtures beyond the basic/direction cases.
- No Rust-side golden regeneration tool; Python remains the source of truth.
