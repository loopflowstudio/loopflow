## Try it!

```bash
cargo test --all
uv run pytest python/tests/
grep -R "gstack/projects" .lf/steps/gstack | wc -l
cargo run --quiet --bin lf -- --list | sed -n '30,40p'
```

What to look for:
- Rust and Python suites pass.
- The legacy `~/.gstack/projects/$SLUG/` path no longer appears in imported gstack step files (`0` matches).
- `lf --list` shows the new custom flows:
  - `gstack-plan-manual`
  - `gstack-review    [and] → gstack:review → gstack:cso → gstack:codex → gstack:review-synthesize`
  - `gstack-sprint    gstack:office-hours → [xor] → gstack-plan-manual → ...`

## Intent

Import the gstack workstyle into loopflow in a way that actually composes with native loopflow steps and flows. That means rewriting gstack's artifact conventions onto loopflow's `scratch/` and `.gstack/` layout, exposing imported prompts as namespaced `gstack:*` steps, defining reusable gstack flows, and teaching the flow engine about custom `and` synthesis so the review fan-out can reconcile into a gstack-specific synthesis step.

## Assumptions

- Imported gstack prompts should be treated as loopflow-native assets, not run through a compatibility shim for the old `~/.gstack/projects/$SLUG/` layout.
- Review fan-out always needs a synthesis step after parallel branches; when a flow provides `synthesize`, that explicit step should win over the builtin fallback.
- Reviewers will validate discovery/listing against the branch build (`cargo run --bin lf -- --list`), not a previously installed `lf` binary.

## Key decisions

- Added a post-import conversion pass in Python rather than hand-maintaining imported prompts.
- Stored workstyle steps under `.lf/steps/gstack/` and workstyle flows under `.lf/flows/gstack/` so they behave like first-class loopflow assets.
- Added `synthesize: Option<String>` to `and` flow items instead of special-casing gstack review behavior elsewhere.
- Polished flow preview/list rendering to include the synth step, so the displayed flow matches runtime behavior.

## Not included

- Browser helper/binary packaging for browser-oriented gstack steps.
- Additional runboard work outside the gstack import/flow scope.
- Any compatibility layer for legacy gstack artifact paths.
