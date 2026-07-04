# Rebase notes / assumptions (2026-07-04)

## `lf runs` / `lf trace` preservation (decision, not a blocker)

- Rebasing `jack-heart.goals` onto `origin/main` (`a4bef310a`).
- `a4bef310a` (#797, the rebase target tip) added `lf runs` + `lf trace`
  (local run ledger, migration 047, `commands/runs.rs`). This landed on main
  **after** this branch forked — `runs.rs` is absent at the merge-base.
- The branch never touched that feature; its scratch notes
  (`wave-next-steps.md` item 5, "ledger convergence") anticipate it arriving
  via main later.
- The rebase auto-merge of `lf/mod.rs` + `commands/mod.rs` silently DROPPED
  the feature (enum variants, `pub mod runs;`, `commands/runs.rs`, dispatch).
  Dropping it would regress main on merge.
- **Decision:** preserve `lf runs`/`lf trace`. Resolve the `lf.rs`/`lf/mod.rs`
  conflicts toward the branch's structure during the rebase, then restore
  main's runs/trace (file + module + enum + dispatch + run_label +
  KNOWN_COMMANDS) as a reconciliation pass, and `cargo build` to verify.
