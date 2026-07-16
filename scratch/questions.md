# Open questions / assumptions

## 2026-07-16 — W2-253 state preservation after OpenCode prompt-prep stall

Incoming coordination note: "OpenCode is stalled before first output at prompt
preparation. Preserve all Task state; the portfolio driver is handing active
dependency fronts to Claude and will resume children after their parent lands."

Interpreted "Preserve all Task state" as: checkpoint the dirty worktree into a
durable commit so the stall cannot orphan the work. The tree held W2-253 (Task PR
operations fail closed without durable registry authority) uncommitted across
`task.rs`, `store/mod.rs`, and a new 508-line test suite.

Actions taken (all reversible, no external side effects):
- Verified sound: `cargo check`, 8/8 `task_pr_authority_tests` pass, `cargo
  clippy --tests -- -D warnings` clean, `cargo fmt --all -- --check` clean.
- Committed locally as `8f28363c6` — **did not push or open a PR**. The portfolio
  driver is handing dependency fronts to Claude and will resume children after
  the parent lands, so landing/publishing is left to that driver, not taken here.

If the driver instead wanted the work left dirty (uncommitted) for Claude to
pick up in-place, the commit is trivially soft-reset — no state was lost either
way. Branch is behind `origin/main` by 15; a rebase will be needed before any
publish.
