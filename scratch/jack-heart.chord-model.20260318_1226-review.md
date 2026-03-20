# Review — jack-heart.chord-model.20260318_1226

## What was implemented

- Reworked the built-in flow model so `xor` is the single-path branching construct, `or` is reserved for future multi-select branching, `loop` is a first-class flow item, and `op:` is the canonical mechanical-ops syntax.
- Reorganized built-in flows and steps around the shipped execution model: `tend/*` scan/assess became `garden/*`, chord mutation/review became `wave/mutate` and `wave/review`, and VSM governance became four explicit scan/assess/mutate flows.
- Updated the CLI, daemon executor, Python model parser, redesign bootstrap script, README, built-in flow docs, and wave planning docs so the parser, runtime, and documentation all describe the same routing structure.

## Key choices

- Kept `xor` and `or` structurally similar in the parser, but only shipped `xor` execution now. That preserves a clean model for future multi-select work without overloading single-path branching.
- Moved chord mutation to `wave/mutate` and retrospective checking to `wave/review` instead of keeping the older draft/review/apply trio. The runtime now reflects the actual two-step governance cycle.
- Split VSM governance into `govern-identity`, `govern-intelligence`, `govern-control`, and `govern-coordination` so each system function has its own scan/assess pair before converging on the same mutation step.
- Updated the redesign bootstrap to configure the redesign wave as `garden-or-silent` over the `wave/chord-model/` and `wave/agent-embedding/` areas, matching the new shipped chord topology.

## How it fits together

The flow parser expands YAML into concrete items (`step`, `op`, `and`, `xor`, `or`, `loop`) and both `lf flow` and the daemon executor consume the same concrete representation. Built-in flows now use that representation directly: garden and VSM scan/assess flows route through `xor` into `wave/mutate`, while higher-level build flows use `loop(..., exit: xor(...))` to model iterative build/review cycles. The docs and bootstrap script were updated to mirror those runtime names so reviewers can read the shipped model straight from the repo.

## Risks and bottlenecks

- `or` is parsed, expanded, and rendered, but execution still intentionally errors in both the CLI and daemon. That is safe for this branch because shipped built-ins only use `xor`, but future `or` adopters need execution support before they can rely on it.
- Routing still depends on prompt behavior and scratch-file verdicts (`scratch/route-xor.md`), so this branch is structurally well-tested rather than end-to-end proven against live agent runs.
- The branch renames several public built-in flow and step names. Repo docs and tests are aligned, but out-of-tree users with hard-coded old names will need to update.

## What's not included

- Multi-select `or` execution.
- Worker-pool execution and non-`manual` chord wave modes.
- Concrete chord member-wave configs for VSM s5/s4/s3/s2/s1 roles.
- Concurrent ingest / atomic claiming work.

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
  - Passed: Rust unit, integration, and doc tests including `flow_tests`
- `uv run pytest python/tests/`
  - Passed: `115 passed`
