## Try it!
- `cargo test -p loopflow --test flow_tests`
- `cargo test -p loopflow lf::commands::flow::tests`
- `cargo test --all`
- `uv run pytest python/tests/`
- Inspect the new tend flow surface with `lfq show <wave> --json`, `lf flow tend`, and the built-in tend docs under `rust/loopflow/src/engine/builtins/steps/tend/`.

What you should see:
- tend parses as `scan-waves -> or(router: tend/assess)` with `chord`, `reorg`, and `silence` paths
- `ship-roadmap` keeps working with ops inside an `or` sub-flow
- Python and docs no longer expose standalone chord CRUD

## Intent
Make the redesign chord real by collapsing chords into ordinary waves, wiring the tend built-ins into the flow engine/runtime, and bootstrapping the new wave-based coordination model in code and docs. The branch makes tend structurally executable now, while leaving the first live redesign tend cycle as a follow-up against a running lfd.

## Assumptions
- `wave/<name>/<name>.yaml` is the source of truth for wave identity and configuration.
- Tend agents can shell out to `lfq show <wave> --json` and parse the existing wave DTO shape instead of needing a new API.
- `or` router steps write `scratch/route-or.md` with a first line of `path: <key>`.

## Key decisions
- Deleted chord CRUD end-to-end instead of keeping compatibility shims; chord-waves reuse the existing wave model.
- Added `and`/`or` as the explicit flow vocabulary and validated `or` sub-flows structurally in Rust tests.
- Hardened CLI `or` execution so the temporary `or-route` step always restores any pre-existing repo-local step file after the run.
- Kept the `lf ops` naming as-is for this milestone to avoid mixing a broad mechanical rename into the tend wiring diff.

## Not included
- No live `lf tend` execution against a registered redesign wave yet.
- No Letta integration.
- No automated scheduling/looping for tend beyond the built-in flow definitions.
