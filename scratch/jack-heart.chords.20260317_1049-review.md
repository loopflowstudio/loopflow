# Branch Review — jack-heart.chords.20260317_1049

## What was implemented
- Replaced standalone chord CRUD with chord-waves: redesign and member coordination now live in wave YAML, wave docs, and existing wave APIs instead of separate chord tables/routes/models.
- Added tend as a first-class flow: built-in tend steps, tend/reorg flow YAML, flow-engine support for `and`/`or`, CLI execution for `or` routing, and flow tests that cover tend structure plus ops inside `or` sub-flows.
- Expanded runtime and docs around the redesign bootstrap: `lfq show --json` now has matching scan guidance, wave DTO/CLI output surfaces flow/area/runtime details, bootstrap-redesign script seeds the redesign/member waves, and the new wave directories document the roadmap.

## Key choices
- **Chord = wave, not a parallel resource.** The branch deletes chord tables, DTOs, routes, Python models, and client methods instead of keeping compatibility shims. The wave config on disk is the source of truth.
- **Use `lfq show --json` for tend runtime state.** The scan prompt points agents at the existing CLI/HTTP contract instead of adding a new aggregation endpoint.
- **Make `or` executable in both parser and CLI.** The flow engine now parses/expands `and` and `or`, validates `or` sub-flows, and the CLI runner executes router steps inline. This gate pass also hardens the temporary router-step file so existing `.lf/steps/or-route.md` content is restored after a run.
- **Keep live tend-cycle validation separate from wiring.** Structural tests now pass, but the first real redesign tend cycle still needs a live lfd instance and manual review.

## How it fits together
The redesign chord-wave is now just another wave whose `area` points at member-wave directories under `wave/`. Tend reads those wave docs plus live `lfq show --json` state, routes through `tend/assess`, and either composes mutations (`tend-chord`) or runs a coherence pass (`reorg`). The daemon, CLI, Python client, and docs all converge on that single model: waves are the only coordination primitive.

## Risks and bottlenecks
- **Live integration still open.** I did not start lfd and register the redesign wave in this gate pass, so the first real `lf tend` cycle remains unverified end-to-end.
- **Large conceptual diff.** Reviewers should pay attention to the flow-language rename (`fork/branch` → `and/or`) and chord-resource deletion together, because they shift both API surface and operator vocabulary.
- **Router-step behavior depends on scratch artifacts.** `or` routing is now structurally tested, but real runs still depend on router steps writing `scratch/route-or.md` in the documented format.

## What's not included
- No live redesign tend-cycle execution against a running lfd.
- No `lf ops` → `lf op` rename.
- No Letta integration or automated tend scheduling beyond the new built-ins/docs.

## Validation
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `cargo test -p loopflow --test flow_tests`
- `cargo test -p loopflow lf::commands::flow::tests`

## Done-when status
- ✅ `scan-waves.md` reads lfd state via `lfq show --json` and emits a runtime section.
- ✅ Flow structure/expansion coverage exists for tend and ops-in-`or` sub-flows.
- ⚠️ First live `lf tend` run against redesign is still outstanding; tracked in `scratch/questions.md`.
