# Finish product PR #855

## Intent

Make the current product branch ready for human review without landing its PR.
Use this child only to review the complete `main...jack-heart/product` diff,
repair concrete defects, and return verified corrections to `jack-heart/product`.

## Scope

1. Converge this child onto the latest local `jack-heart/product` with
   `lf rebase jack-heart/product`. Recompute the diff against current main after
   convergence; do not assess the stale child snapshot.
2. Review every changed file in the parent diff as one release unit:
   - the asynchronous shared GUI process environment and every Swift launcher
     that consumes it;
   - development and release app assembly, where the SwiftPM product
     `LoopflowMac` must be installed as the bundle executable `Loopflow`;
   - migration `057_runs_step_index_repair`, including fresh, affected
     pre-release, already-converged, and repeated-apply behavior;
   - the product objective, daily wave cron, seven-project roster, and the
     auditability, distributed-computing, and performance proof contracts.
3. Apply only findings that can cause incorrect behavior, schema convergence,
   bundle assembly, test instability, or a non-computable product contract.
   Keep unrelated cleanup and new product behavior out of this child.
4. Do not read, infer, create, or update Linear state. The expired token is an
   external limitation, not permission to reconstruct PM state locally.

## Verification

- Rust: run the focused migration tests, then `cargo fmt --check`,
  `cargo clippy -- -D warnings`, and the applicable `loopflow` test suite. The
  evidence must show that an affected ledger regains `runs.step_index`, loses
  the erroneous `runs.skill_index`, records migration 057, and remains valid on
  a second migration pass; a fresh or already-correct ledger must also converge.
- Python: run the focused install/release automation tests and compile-check the
  two changed scripts. Assert the built SwiftPM source name and the installed
  `CFBundleExecutable` path agree for both development and release assembly.
- Swift: run `BundledDaemonPathTests`, `WavePlanParserTests`, and the full Swift
  package suite with `-Xswiftc -gnone`. Because Mac launch paths changed, also
  run the Loopflow macOS xcodebuild test command from `TESTING.md` when the
  local Xcode environment supports it.
- Contracts: parse `wave/product/GOAL.md` through the repository's wave config
  path and confirm the daily `wave` cron is accepted. Confirm `projects/`
  remains the seven-file roster and each changed KR is a measurable proof, not
  a task list or implementation receipt.
- Run `uv run python scripts/test.py --list` after convergence and execute any
  additional changed-aware suite it identifies.

## Review standard

Read the full diff in the repository review ritual's spirit: one clear owner
per behavior, boring failure modes, explicit diagnostics, no duplicate wire or
process-environment implementation, and documentation that matches what ships.
Record each concrete finding with its file and consequence, then fix it. If no
finding survives verification, record that outcome with the commands run.

## Landing

Commit only child review fixes and evidence. Land this child into
`jack-heart/product` through `lf pr land`; do not submit, auto-merge, close, or
otherwise land parent PR #855. Confirm #855 remains open after the child lands.
Post the result to the product wave with the child PR/commit and test receipts.

If convergence cannot complete, write an actionable non-convergence record to
`scratch/questions.md`: failing command, expected and actual state, current and
target commits, files or tests still unresolved, and the smallest next action.
Report that record to the product wave without changing Linear.

## Done when

- The latest parent diff has a complete review receipt and no unresolved
  concrete findings.
- Proportional Rust, Python, and Swift checks pass, including the migration's
  affected-ledger recovery path.
- Any child corrections are landed into `jack-heart/product`.
- Parent PR #855 is still open and unlanded, and no Linear state changed.
