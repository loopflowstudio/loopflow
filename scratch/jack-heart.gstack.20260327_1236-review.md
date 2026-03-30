# gstack stage 2 review

## What was implemented

Imported the gstack workstyle into loopflow with namespaced steps under `.lf/steps/gstack/`, namespaced flows under `.lf/flows/gstack/`, and a Python converter that rewrites upstream prompt content into loopflow conventions. The Rust flow engine now supports optional custom `synthesize` steps on `and` branches, and discovery/listing surfaces show the new gstack flows and steps. This gate pass also polished the flow/list rendering so `gstack-review` visibly ends with `gstack:review-synthesize` instead of hiding the synthesis step.

## Key choices

- Rewrote imported gstack artifacts to loopflow-standard paths (`scratch/` and `.gstack/`) instead of keeping `~/.gstack/projects/$SLUG/` compatibility shims.
- Modeled imported gstack prompts as namespaced external skills (`gstack:*`) and gstack flows as repo-local namespaced flows (`gstack-review`, `gstack-sprint`, `gstack-plan-manual`).
- Added `synthesize: <step>` to `and` flow items so review fan-out can use a workstyle-specific synthesis prompt instead of the builtin fallback.
- Kept the converter responsible for the mechanical prompt/path rewrites, with tests covering the imported-workstyle transformation path.
- Polished flow discovery/rendering so reviewer-facing output matches execution: the custom synth step now appears in `lf --list` and flow pipeline previews.

## How it fits together

The converter ingests upstream gstack skills and emits loopflow-native step files plus workstyle metadata. The Rust discovery layer surfaces those namespaced steps and flows to `lf`, while the flow engine parses and executes the new `and.synthesize` field so the gstack review flow can fan out to `review`, `cso`, and `codex` before reconciling them with `review-synthesize`.

## Risks and bottlenecks

- Imported prompt content is large and mechanically transformed; regressions are more likely to be in prompt wording/path rewrites than in engine code.
- Browser-oriented gstack steps still depend on separate browser asset packaging/runtime support; that remains the main follow-on risk outside this branch's core flow/import work.
- `lf --list` validation must be run against the branch build (`cargo run --bin lf -- --list`), not an older globally installed `lf`, or the new flows may appear missing.

## What's not included

- No browser daemon/helper asset packaging for the gstack browser steps.
- No Codex/OpenCode health-adapter work from the runboard wave.
- No backwards-compatibility layer for the old `~/.gstack/projects/$SLUG/` artifact layout.

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `grep -R "gstack/projects" .lf/steps/gstack | wc -l` → `0`
- `cargo run --quiet --bin lf -- --list` shows:
  - `gstack-plan-manual`
  - `gstack-review    [and] → gstack:review → gstack:cso → gstack:codex → gstack:review-synthesize`
  - `gstack-sprint    gstack:office-hours → [xor] → ...`
