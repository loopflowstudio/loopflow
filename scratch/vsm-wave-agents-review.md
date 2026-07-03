# vsm-wave-agents Review

## What was implemented

Added five builtin VSM system goal charters under `govern/goal/` and exposed them through `lf goal <wave> --system s1..s5`. The command keeps the target wave's context, memory, metrics, flows, and in-flight work, but swaps the goal body to the selected builtin system charter.

Gate polish added two pieces:

- `--system` loads embedded builtin goal text directly, so repo-local `.lf/goals/govern-control.md` cannot shadow the shipped VSM charter.
- `README.md` and `docs/wave-authoring.md` now show the `lf goal <wave> --system s3 --once` path.

## Key choices

The system flag maps only the VSM shorthand names: `s1` through `s5`. That keeps the public CLI small and leaves the longer `govern-*` names as implementation identifiers.

Builtin system goals deliberately bypass repo goal overrides. Normal `lf goal <wave>` still uses the existing `load_goal` precedence; system charters are different because their exact wording is the artifact being shipped.

No new DTOs or runtime types were added. The feature stays in the existing `Goal { prompt }` path and the existing goal renderer.

## How it fits together

`lf goal` resolves the wave name first, then optionally resolves `--system` to a builtin govern goal key. `build_goal_message` renders either the wave's own `GOAL.md` or the selected builtin charter, while `read_wave_config` and `MEMORY.md` continue to use the target wave name.

The builtin registration remains automatic through `build.rs`; dropping the five markdown files under `rust/loopflow/src/engine/builtins/govern/goal/` makes them resolvable by the generated builtin goal map.

## Risks and bottlenecks

The `--system` accepted values are lowercase only. That matches the design, but users typing `S3` will get the explicit invalid-system error.

The smoke command currently halts for `root` because `root` has no roadmap handle or metrics configured. That verifies the command launches the S3 charter and one-iteration stop path, but not a dispatching loop with real roadmap work.

## What's not included

The existing `govern-*` flows were not changed. No scheduler, lfd wave execution, PM sync, or UI behavior was added for standing VSM system loops. No aliases beyond `s1` through `s5` were added.

## Validation

```bash
cargo fmt --all -- --check
cargo build
cargo clippy --all-targets -- -D warnings
cargo test --all
uv run ruff check python/loopflow python/tests
uv run pytest python/tests/
tests/e2e/test_smoke.sh
perl -e 'alarm shift; exec @ARGV' 30 cargo run -q -p loopflow --bin lf -- goal root --system s3 --once
git diff --check main...HEAD
```

The smoke command completed one iteration and halted because `root` has no roadmap handle or metrics to act on.
