## Try it!

```bash
cargo fmt --check
cargo clippy -- -D warnings
uv run python scripts/test.py
```

The changed-aware runner selects Rust for this branch. It passed with 1,239
tests, 3 skipped.

Full matrix was also attempted:

```bash
uv run python scripts/test.py --all
```

Python, Rust, website, and Swift package tests passed. The Concerto Xcode UI job
hung after building the UI runner and was interrupted; result bundle:
`/Users/jack/Library/Developer/Xcode/DerivedData/LoopflowSwift-edortmcwhqfgbybchhwnmviqshkn/Logs/Test/Test-Concerto-2026.07.06_18-47-44--0700.xcresult`.

## Intent

Prepare worker lifecycle dispatch for parent-targeted workers. A fresh worker
now forks from the owning wave branch when it exists and records that branch as
its review target, while bare waves still dispatch from the default branch.

## Assumptions

Fresh dispatch may fall back to the default branch when the wave parent branch
does not exist yet. Explicit stack dispatch is stricter: a missing parent run or
parent branch remains an error.

## Key decisions

- Keep every worker on its own stamped branch instead of checking out the target
  branch directly.
- Share the local-vs-remote parent resolver between fresh and stacked worktrees.
- Prefer the local parent tip unless it is strictly behind `origin/<parent>`.
- Remove the committed `.lf/scratch-stash` duplicate so the PR has one design
  note to review.

## Not included

This is only the dispatch-targeting slice. Exec-in-worker, the minded
terminating worker, cascade/sealing, prod verification, and HITL attach remain
future slices.
