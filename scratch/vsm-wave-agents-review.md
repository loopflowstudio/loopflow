# vsm-wave-agents Review

## What was implemented

Added five builtin Viable System Model system charters as goals named `s1` through `s5`. Each charter is a body-only markdown builtin under `rust/loopflow/src/engine/builtins/govern/goal/`, so `lf goal s3 --once` reaches the S3 control charter through the existing goal loader.

Gate polish kept the simpler implementation from the second pass: no `--system` flag, no VSM-specific goal mapping, and no special launch path. The docs and tests now describe and pin the builtin-goal behavior directly.

## Key choices

The public names are `s1` through `s5`, not `govern-control` style aliases. That makes the VSM systems short builtin goals while leaving the existing `govern-*` flows as separate hands for operational work.

`lf goal` was left generic. `resolve_wave_name` accepts an explicit name, and `load_goal` already searches repo overrides, `wave/<name>/GOAL.md`, then builtins. That means a future `wave/s3/GOAL.md` intentionally overrides the builtin, using the same precedence as every other goal.

The shipped charters remain body-only markdown. No DTOs, runtime types, scheduler behavior, or launch semantics changed.

## How it fits together

`build.rs` scans `builtins/*/goal/*.md` and registers core-category goals in the flat builtin goal namespace. The five `s1`...`s5` files therefore become builtin goal keys.

`lf goal s3 --once` normalizes `s3` as the wave/goal name, finds no local wave goal in this repo, loads the builtin `s3` goal, renders it with empty `s3` wave context, and launches the standard goal prompt with the one-iteration marker.

## Risks and bottlenecks

Repo-local goal overrides can shadow these builtins by name. That is consistent with the loader, but reviewers should notice that the exact shipped wording is only guaranteed when no `.lf/goals/s3.md` or `wave/s3/GOAL.md` override exists.

The smoke command halts because this repo has no `wave/s3` roadmap, metrics, memory, or in-flight work. That verifies launch and stop behavior, not a productive standing VSM loop with real roadmap input.

## What's not included

No `--system` flag. No wave directories for `s1` through `s5`. No scheduler, lfd activation, UI, PM sync, or changes to the existing `govern-*` flows.

## Validation

```bash
cargo fmt --all -- --check
cargo build
cargo clippy --all-targets -- -D warnings
cargo test -p loopflow
cargo test --all
uv run ruff check python/loopflow python/tests
uv run pytest python/tests/
tests/e2e/test_smoke.sh
perl -e 'alarm shift; exec @ARGV' 30 cargo run -q -p loopflow --bin lf -- goal s3 --once
git diff --check main...HEAD && git diff --check
```

The one-shot smoke command completed and halted cleanly on empty `s3` context.
