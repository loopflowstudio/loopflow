# lf command API redesign review

## What was implemented

Promoted the mechanical `lf op ...` surface into first-class commands:
`lf pr`, `lf wt`, `lf rebase`, `lf commit`, `lf auth`, `lf release`, and
`lf pm`. Flow operation items still use the `op:` step marker, but their
payload now follows the same grammar as humans type, e.g.
`op: pr land --create-pr`.

The branch was rebased onto `origin/main` at `601881cd`, carrying forward
main's newer Linear task/project PM surface under `lf pm`. Docs, builtins,
website docs, scripts, tests, and prompt goldens were swept so current-facing
guidance no longer tells agents or users to run `lf op`.

Gate polish also fixed a live Swift break: Loopflow session command assembly
still emitted `lf op commit --push`; it now emits `lf commit --push`. The wave
exec-door policy now explicitly tests the denied `task` and hidden
`sync-skills` verbs.

## Key choices

- Kept `op:` as the flow-step type. The redesign removes the CLI drawer, not
  the engine's flow operation marker.
- Made bare `lf pr` show PR status, with `lf pr open`, `lf pr submit`, and
  `lf pr land` carrying lifecycle verbs.
- Preserved main's PM task/project additions (`pm task ...`, `--project`,
  `--pr`, and `pm sync --plan`) under the promoted `lf pm` command.
- Deleted the retired human CLI verbs: `next`, `advance`, `branches`, `sync`,
  `doctor`, `shell`, `cp`, `push`, and `queue reconcile`. `sync-skills`
  remains hidden because install/sync verification scripts invoke it directly.

## How it fits together

`lf/mod.rs` defines the promoted clap grammar. `bin/lf.rs` routes each command
to the existing ops implementation, and `ops/flow.rs` parses `op:` flow items
through the same CLI grammar before dispatching supported mechanical actions.
Machine paths that used to shell through stale CLI argv now call library code
or validate against the new grammar.

## Risks and bottlenecks

- There is no `lf op` compatibility shim. External automation not swept by this
  PR will fail fast.
- Historical release notes and recorded fixtures still contain `lf op` because
  they describe past behavior or captured data.
- `op: rebase --plan` in a flow still returns success without printing a plan;
  no builtin flow uses it, but a future pass should reject that payload rather
  than treating it as a no-op.

## What's not included

- No migration layer for old flow payloads.
- No rewrite of historical release artifacts.
- No push/force-push after the rebase; the local branch is ahead of and behind
  its old remote ref until a human updates the PR branch.

## Validation

- `cargo test -p loopflow wave_exec_policy --lib` - pass
- `cargo fmt --all -- --check` - pass
- `cargo clippy --all-targets -- -D warnings` - pass
- `cargo test` - pass
- `swift test --package-path swift -Xswiftc -gnone` - pass
- `uv run pytest python/tests/` - pass
- `uv run python scripts/test.py --all` - Python pass; Rust pass; website pass;
  Swift package pass; e2e smoke pass; Loopflow UI failed in this headless
  session. The first run hit a stale DerivedData linker write (`LoopflowUITests`
  output file); after `xcodebuild clean`, the rerun built and passed app/unit
  tests, then `LoopflowUITests-Runner` exited before establishing the UI-test
  connection.
- `cd swift && xcodegen generate && xcodebuild test ...` after clean - same
  Loopflow UI runner early-exit failure; no lingering runner process remained.
- CLI smoke: `lf --help` lists the promoted commands; `lf pm --help` exposes
  main's task/project subcommands under `lf pm`.
