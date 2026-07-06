## Try it!

```bash
lf op wt create bugs --plan
lf op wt create fix-auth --child bugs --plan
lf op wt list
lf op wt switch fix-auth
lf op wt up
lf op wt down fix-auth
```

Expected shape: sibling worktrees root from main by default, child worktrees require `--child`, local directories are flat (`loopflow.bugs.fix-auth`), and branches are author-scoped (`<user>/bugs.fix-auth`).

Validation run:

```bash
cargo fmt --check
cargo clippy -- -D warnings
uv run python scripts/test.py
uv run python scripts/test.py --all
```

Changed-aware tests passed. The full matrix passed Python, Rust, website, Swift, and e2e. Concerto's default DerivedData run hit an Xcode cache/link write failure; a clean derived-data rerun got past the link step and passed the unit layer, then the UI-test runner was killed before bootstrapping in this headless environment.

## Intent

Make worktree and branch identity deterministic enough for recursive waves: one identity model, two explicit projections, no schema grammar, no post-land worktree rotation.

## Assumptions

Dots are reserved for chain ancestry. `/` scopes the remote branch by user and never appears in worktree directory names. Wave homes are persistent; ephemeral worker cleanup belongs to the worker lifecycle, not `lf op land`.

## Key decisions

`WaveId` is the single parser/emitter for wave, subwave, and worker names. `lf op wt create` now creates a sibling from main unless `--child` is passed, so ad-hoc worktrees do not accidentally stack under the current branch. `lf op land` and `lf op submit` prepare or finalize PRs but leave the live worktree in place.

## Not included

Worker supervisor/runtime, retirement of `lf op next` and the remaining advance endpoints, and migration tooling for old branch-schema worktrees.
