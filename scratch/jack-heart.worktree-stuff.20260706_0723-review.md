# Gate review: worktree identity redesign

## What was implemented

Added a fixed `WaveId` identity model for wave, subwave, and worker naming. Local worktree directories now use flat, author-free components like `bugs.fix-auth.20260706_0801`, while remote branches use author-scoped names like `jack/bugs.fix-auth.20260706_0801`.

`lf op wt create` now defaults to sibling/root placement from main, with explicit child placement through `--child`. `lf op wt switch`, `up`, `down`, and `list` understand the chain model. Land and submit no longer rotate or rename the active worktree.

## Key choices

- Removed configurable `branch_names` schemas. The old schema grammar let root formatting and stack ancestry fight over dots; the new model has one parser and two explicit projections.
- Kept dots as chain separators and `/` as the remote-only author separator. The slash never reaches filesystem paths.
- Made wave homes persistent. `lf op land` prepares or arms PR landing but leaves the worktree in place; worker cleanup belongs to the worker lifecycle.
- Updated user docs to remove the stale branch-name schema example and the old `--stack` wording in worktree placement docs.

## How it fits together

`engine::identity::WaveId` owns parsing and emission for branch/worktree identity. `engine::worktrees` consumes it for directory planning, wave worktree creation, worker worktree creation, placement planning, and list/switch behavior. `ops::land` now stops after PR finalization, so `lf::commands::ops` no longer emits a post-land `cd`.

## Risks and bottlenecks

- Existing local branches using old schema shapes may not map cleanly into `WaveId`; exact branch matching still handles known worktrees, but new creation is intentionally on the fixed model.
- The branch still carries old `lf --stack` / `--fork` dispatch behavior for run placement; this pass only changed `lf op wt create` placement.
- Concerto UI tests need a graphical/bootstrap-capable environment. In this headless run, the UI test runner was killed before bootstrapping after the clean rebuild succeeded.

## What's not included

- Worker supervisor/runtime implementation.
- Retiring `lf op next` and the remaining wire/UI advance paths.
- Migration tooling for old locally-created branch-schema worktrees.

## Validation

- `cargo fmt --check` — passed.
- `cargo clippy -- -D warnings` — passed.
- `uv run python scripts/test.py` — passed changed-aware gate: Rust 1219 passed / 3 skipped; website 61 passed / 3 skipped.
- `uv run python scripts/test.py --all` — Python, Rust, website, Swift, and e2e passed; Concerto failed in the default Xcode DerivedData path with `can't write output file` for `ConcertoUITests`.
- `cd swift && xcodegen generate && xcodebuild test ... -derivedDataPath ../.lf/tmp/xcode-derived-gate` — clean Concerto rebuild got past the linker failure and passed the unit layer, then the UI-test runner exited early before bootstrapping. This matches the no-rendering/headless environment constraint.
