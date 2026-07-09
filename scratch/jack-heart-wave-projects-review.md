# jack-heart/wave-projects review

## What was implemented

This branch makes Loopflow's planning model explicit across code, prompts, docs,
and wave files:

- Waves are durable operating contexts under `wave/<wave>/`.
- Projects are measured bets under `wave/<wave>/projects/<slug>.md`.
- Tasks live in Linear and attach to local projects with `project:<slug>` labels.
- `lf op pm` now exposes task/project operations: `status`, `show --project`,
  `task create/update/done/move`, `rename`, `doctor`, and `sync --plan`.

Gate polish also aligned core built-in prompts and architecture docs with the
new `lf op pm task ...` surface, then refreshed prompt goldens.

## Key choices

One Linear project backs each wave. Local Loopflow projects are labels inside
that Linear project instead of additional Linear projects; this keeps Linear as
the single task container while preserving Loopflow's shallow wave/project/task
model.

`lf op pm update` remains as a compatibility alias, but the user-facing docs and
prompts now teach `lf op pm task ...`.

`sync --plan` diagnoses ambiguous drift instead of guessing task moves. Low-risk
actions like missing labels and Linear-project renames are reported explicitly.

## How it fits together

The CLI parses the new PM subcommands in `rust/loopflow/src/lf/mod.rs` and
prints grouped task output in `rust/loopflow/src/lf/commands/ops/mod.rs`.
Provider-neutral PM operations live in `rust/loopflow/src/ops/pm.rs`; the Linear
adapter in `rust/loopflow/src/lfd/pm/linear.rs` supplies labels, project rename,
task move, and task updates.

Wave docs now carry the durable operating/project state. Linear carries mutable
task state. The bridge is a label named `project:<slug>`.

## Risks and bottlenecks

Live Linear mutation demos were not run during gate because they would create,
move, close, label, or rename real Linear objects. The mocked Linear tests cover
the GraphQL request shapes and task grouping behavior.

Task moves add the destination project label but do not remove stale
`project:<old>` labels. That is intentionally listed as out of scope in the PR
copy.

The full local matrix still cannot green the Loopflow UI command headlessly:
the app/unit bundle reports 304 passing tests, then `LoopflowUITests-Runner`
exits before establishing the UI-test connection and Xcode returns 65. A direct
retry with fresh DerivedData got past a shared-cache linker failure and hit that
same runner failure.

## What's not included

No automatic migration of ambiguous existing Linear tasks. No stale-label
removal during task moves. No new PM provider. No Swift/UI behavior change.

## Validation

Passed:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test -p loopflow pm`
- `cargo test -p loopflow golden_prompt`
- `uv run python scripts/test.py` (Rust + website changed-aware suites)
- `uv run python scripts/test.py --all`: Python, Rust, website, Swift package,
  and e2e suites passed.

Blocked locally:

- `uv run python scripts/test.py --all`: Loopflow UI suite failed in shared
  DerivedData with `can't write output file` for `LoopflowUITests`.
- Fresh DerivedData UI retry:
  `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme LoopflowMac -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -derivedDataPath /tmp/loopflow-gate-dd-1783566501`
  built and ran the app/unit bundle; 304 tests passed, then
  `LoopflowUITests-Runner` exited before establishing connection.
