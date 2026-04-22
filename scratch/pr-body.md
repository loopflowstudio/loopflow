## Try it!

```bash
cargo run -q -p loopflow --bin lf -- --list
cargo run -q -p loopflow --bin lf -- op gstack list
uv run pytest python/tests/test_workstyle_convert.py -q
cargo test -p loopflow --test discovery_tests -- --nocapture
cargo test -p loopflow --test flow_tests -- --nocapture
```

You should see gstack flows (`gstack-sprint`, `gstack-review`, `gstack-plan-manual`), 37 imported `gstack:*` steps, and passing converter/discovery tests.

## Intent

Bring Garry Tan's gstack workflow into loopflow as an imported workstyle. The change adds generated gstack steps, namespaced flow/step discovery, sync/list/diff ops, and starter wave docs so teams can run gstack-style sprint and review flows without copying prompts by hand.

## Assumptions

- Imported workstyles live under `.lf/steps/<prefix>/` and are invoked as `<prefix>:<step>`.
- Gstack sync can rely on `git`, network access to `garrytan/gstack`, and `uv run python -m loopflow.workstyle.convert`.
- Reviewers care most about converter behavior and resolution semantics. The imported prompt bodies are generated artifacts.

## Key decisions

- Strip upstream telemetry, preamble wrappers, and Claude-specific integration sections during conversion.
- Preserve colon syntax for steps and hyphenated names for flows so `lf --list` stays readable.
- Use the built-in `synthesize` step for `gstack-review`; there is no generated `gstack:review-synthesize` step.
- Clean built-in direction text to avoid identity framing while preserving the source intent.

## Not included

- Scheduled upstream sync.
- Full native rewrite of every imported gstack prompt.
- Backward compatibility for alternate upstream skill formats not covered by the converter tests.
