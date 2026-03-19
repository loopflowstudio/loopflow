# Review — jack-heart.chord-model.20260318_1226

## What was implemented

- Replaced tend's draft/apply split with `tend/play-chord` plus a retrospective `tend/review-chord`, using inline `steps:` inside `tend.yaml` so the tune path stays self-contained.
- Replaced the old sequential VSM flow with four governance flows (`govern-identity`, `govern-intelligence`, `govern-control`, `govern-coordination`) plus new scan/assess steps for s5/s4/s3/s2.
- Updated flow parsing, builtin registration, tests, bootstrap/docs, and wave planning docs to match the new governance-flow model.

## Key choices

- Added inline `steps:` support on `or` paths instead of keeping a separate `tend-tune` subflow. The routing decision and the follow-on steps now live in one flow file.
- Kept every governance flow ending in `tend/play-chord` so tend and VSM share one mutation point instead of drifting into separate execution paths.
- Left worker pools, wave-mode renaming, concrete VSM member-wave configs, and concurrent ingest as follow-up roadmap items rather than partial implementations.
- Aligned the repo-local `.lf/steps/tend/review-chord.md` override with the new built-in post-play review semantics so local testing does not mask shipped behavior.

## How it fits together

`tend/scan-waves` gathers chord state, `tend/assess` routes to either silence or an inline `tend/play-chord` → `tend/review-chord` sequence, and `play-chord` becomes the shared mutation step. The VSM governance flows split scanning from judgment across s5/s4/s3/s2, then hand their assessments to that same mutation step. Flow parsing now understands inline `or`-path steps, and the builtin-flow tests assert the new tend and governance structures directly.

## Risks and bottlenecks

- Prompt quality is covered structurally, not behaviorally: tests prove the flows load and expand, but real execution still depends on agents interpreting the new scan/assess/play prompts well.
- `tend/play-chord` is now a shared choke point for tend and all governance flows. A bad mutation there would affect every governance path.
- Some scan prompts depend on tools or external signals that may be unavailable in a given runtime, so real runs still need graceful skip behavior.

## What's not included

- Worker pools / `workers: N`
- `flow` replacing `manual`
- Concrete s5/s4/s3/s2/s1 chord member-wave configs
- Concurrent ingest / atomic item claiming

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all` — passed
- `uv run pytest python/tests/` — 115 passed
