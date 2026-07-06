## Try it!

```bash
lf op wt create bugs
lf op wt create fix-auth --child bugs
lf op wt list
lf op wt switch fix-auth
```

Expected shape:

- root wave branch: `jack/bugs`
- child branch: `jack/bugs.fix-auth`
- worktree dirs stay flat and author-free: `loopflow.bugs`, `loopflow.bugs.fix-auth`

Validation run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
uv run python scripts/test.py
uv run python scripts/test.py --all
```

`scripts/test.py --all` passed Python, Rust, website, Swift, and e2e. Concerto UI did not complete locally: the UI runner was killed before bootstrapping once, then a direct rerun failed to overwrite the previous DerivedData test bundle. The app/unit portion passed before the UI-runner failure.

## Intent

Make wave and worker identity recursive without letting branch naming corrupt worktree placement. A wave now has two explicit projections: an author-scoped branch name for remotes and a flat author-free directory component for local sibling worktrees. Land and submit stop rotating the live worktree, so a wave home stays stable while future worker minds own PR lifecycles.

## Assumptions

- `/` is acceptable in remote branch names for author scoping.
- `.` remains the chain separator inside a wave/worker lineage.
- Human `wt create --child` creates persistent subwave-shaped descendants; dispatch/runtime will create stamped workers.
- The Worker-mind runtime is a later subsystem, not part of this mechanical identity pass.

## Key decisions

- Added `WaveId` instead of continuing to infer identity from configurable branch schemas.
- Removed branch-name schema config from worktree creation paths.
- Changed no-flag `lf op wt create` to root from main; stacking now requires `--child`.
- Preserved namespaced upstream branches in worktree listings by stripping only `origin/`.
- Kept `next`/`advance` endpoints for now because Concerto and the wire surface still call them.

## Not included

- Worker-mind runtime.
- Subwave dispatch UX.
- Wire/Swift removal of `next_wave_handler`.
- Full replacement of CI-fix/land orchestration with parent-targeting worker loops.
