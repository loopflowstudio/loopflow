# Gate Review: Rebase Efficiency

## What was implemented

Added placement planning for `lf op wt create`, including dotted stack ancestry,
dot rejection for user-provided worktree segments, `--main`, `--fork`,
`--stack [PARENT]`, and `--plan`.

Added rebase planning for `lf op rebase --plan`, deterministic reset/rebase
classification, scratch stash/restore for disposable reset paths, and local
ops telemetry under `.lf/metrics/ops.jsonl`.

The gate pass fixed classifier drift for branches that are only behind their
base: upstream changes are no longer counted as local authored files. The new
E2E covers the demo path for scratch-only reset, stack placement planning, and
dot rejection.

The gate pass updated `docs/lfop.md`, embedded Loopflow guidance, and prompt
goldens so user docs and agent guidance match the new command surface.

## Key choices

Placement lives in `engine/worktrees` so `lf op wt` and future execution
placement can share one naming rule.

`lf op rebase` now plans before mutating git. Reset is only used for
unprotected disposable cases; protected paths and authored changes continue
through the normal rebase path.

Scratch preservation is directory-copy based. No patch math is attempted in
this branch.

Root branch names still follow the configured branch schema. Stacked children
append the new segment to the parent branch with a dot.

## How it fits together

CLI parsing in `lf/mod.rs` and `lf/commands/ops/mod.rs` builds placement or
rebase requests. `engine/worktrees` plans and applies worktree placement.
`ops/rebase` classifies the branch and chooses reset, parent rebase, direct
rebase, or noop.

Metrics are written from the ops command layer after successful `wt create` and
`rebase` runs. Plan mode prints deterministic text and does not mutate git.

## Risks and bottlenecks

The branch preserves the existing configurable root branch schema, including
schemas that contain dots. Stack parent behavior is still deterministic because
child branches append to the concrete parent branch, but root branch names with
dots can visually resemble stack ancestry.

The scratch stash restore is intentionally simple. It replaces `scratch/` after
reset instead of resolving conflicting scratch edits.

## What's not included

Normal `lf <flow-or-step> --stack|--fork|--dispatch` placement flags are not
implemented in this branch.

`lf op land` no-advance behavior is not changed.

Telemetry is local JSONL only. There is no aggregation or dashboard.

## Validation

Passed:

```bash
cargo fmt --check
cargo nextest run --all
cargo clippy -- -D warnings
uv run pytest python/tests/test_install_script.py
cd website && uv run python dev.py test
tests/e2e/test_rebase_efficiency.sh
cargo run -q -p loopflow --bin lf -- op rebase --help
cargo run -q -p loopflow --bin lf -- op wt create --help
cargo run -q -p loopflow --bin lf -- op rebase --plan
cargo run -q -p loopflow --bin lf -- op wt create --plan gate-doc-smoke
```

Observed plan smoke:

```text
lf op rebase --plan
class: clean_authored
strategy: direct_rebase
agent_launched: false

lf op wt create --plan gate-doc-smoke
strategy: create_stack_child
```
