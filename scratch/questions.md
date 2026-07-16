# W2-224 — state after PR #1012

## Resumed from directive v2 pause (2026-07-16)

Parent W2-206 (PR #944) merged. Branch rebased onto `origin/main`,
squashed to one clean commit (`bc9f90380`), PR published as #1012.

## What this PR ships

- `lfd` binary: `serve` / `install` / `status` / `uninstall`
- `lfd/mod.rs`: durable delivery inbox replacing the old relay/forwarding mode
- `lfd/service.rs`: launchd (macOS) / systemd user (Linux) rendering
- `provider_deliveries` table (migration `0.11.020`)
- Store layer: `store/provider_deliveries.rs` + `store/sqlite/provider_deliveries.rs`
- Integration test rewritten: boots the real binary, proves dedup across restart

## Verified

- `cargo build --bin lfd` clean
- `cargo clippy --lib --bin lfd --tests -- -D warnings` clean
- `cargo fmt --check` clean
- 15 unit tests pass (inbox dedup, outcome mapping, delivery id, bind guard, 401/503, service render, secret omission)
- 1 integration test passes (signed webhook → 200, duplicate → dedup, restart → durable dedup, unsigned → 401)

## Open for follow-on Tasks

- **`pending` re-process path untested.** No test kills a daemon mid-processing to exercise crash recovery (delivery left `pending`, redelivery re-processes).
- **GitHub adapter absent.** Deliberate — the directive allows proving routing with Linear first. The `provider` CHECK constraint already admits `github`.
- **launchd live lifecycle unrun.** The render is test-covered; install/start/restart/status on the maintained Mac is not.
- **Host disk reclamation.** 106 worktrees held ~636 GiB of cargo `target/` dirs (2026-07-16). Disk has since recovered to 114 GiB free, but the underlying issue (no `target/` reclamation for merged-PR worktrees) is unresolved. Wave `infrastructure` now has `pm.linear_team` in GOAL.md, so this can be filed as a Linear task.

## Registry note

`lf pr publish` refused due to a stale `fork_base` (parent PR was squash-merged, so the recorded base diverged from `origin/main`). Branch was pushed directly with `git push` and PR created with `gh pr create`. `lf pr status` recognizes the PR.
