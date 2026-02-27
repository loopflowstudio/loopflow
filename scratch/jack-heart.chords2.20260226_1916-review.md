# Gate Review: Git Sync Hardening Design

## What this branch does

Ingests wave item `036-git-sync-hardening` and produces a design doc (`scratch/harden.md`) for hardening the executor's git sync layer against concurrent push failures.

No code changes. This is a design-only branch.

## Design doc assessment

**Verified against codebase.** Every function, type, trait, and file path referenced in the design doc exists and behaves as described. The gaps identified (pre_step_sync missing upstream rebase, bare rebase() calls, single-retry push failure) are real.

**Well-scoped.** Four changes, all in `helpers.rs` plus one new helper (`run_debug_agent`). No new modules, no new abstractions beyond `TracingProgress` (6 lines) and `dual_rebase` (consolidation of existing logic).

**Clear implementation path.** Each section includes concrete code snippets that match the actual types and signatures in the codebase. An implementer can follow this mechanically.

## Key choices

| Decision | Rationale | Risk |
|----------|-----------|------|
| Agent escalation over retry loops | Structural failures need investigation, not retries | Agent API downtime still kills runs (same as today) |
| Shared `dual_rebase` helper | Eliminates drift between `pre_step_sync` and `sync_existing_worktree` | Mechanical refactor, low risk |
| `TracingProgress` over `NullProgress` | Executor sync is where visibility matters most | Adds 6 lines of code |
| `timeout` on `ProcessConfig` | Prevents infinite agent hangs | See note below |

## Implementation note: timeout mechanism

The design proposes spawning a monitor thread with `unsafe { libc::kill() }`. This has a PID-reuse race: if the child exits before the timeout fires, the PID could be reassigned to another process. During implementation, prefer using the `Child` handle directly (e.g., `child.kill()` via a shared `Arc<Mutex<Child>>`, or a `tokio::time::timeout` wrapper if async context is available). The design intent (prevent infinite hangs) is correct; the mechanism should be refined during implementation.

## What's not included

- No concurrency limiting for listen fan-out (separate concern, explicitly out of scope)
- No changes to `rebase_with_recovery` itself
- No agent rate limiting
- No push retry backoff (agent escalation replaces this)

## Risks

- **Agent API dependency.** Both rebase recovery and push escalation depend on agent sessions. An API outage during sync means the run fails — but this is no worse than today's hard-fail behavior.
- **Latency.** Agent sessions (especially 30-min rebase timeout) add latency to step boundaries when conflicts occur. This is intentional — resolving conflicts is real work.
- **`main_repo` parameter threading.** Requires touching the executor call site to pass `wave.repo` into `pre_step_sync`. Mechanical but touches a hot path.

## Verdict

Design is ready for implementation. All claims verified. Scope is tight. One implementation detail (timeout mechanism) should be refined during the code phase.
