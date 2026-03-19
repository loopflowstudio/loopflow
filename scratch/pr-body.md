## Try it!

- `cargo test --test flow_tests`
  - Confirms the builtin `tend` flow now routes to inline `tend/play-chord` → `tend/review-chord`, and that the four governance flows expand to the expected scan/assess/play sequence.
- `uv run pytest python/tests/test_bootstrap_redesign_script.py -q`
  - Confirms redesign bootstrap still creates/configures waves correctly from a sibling worktree.
- `cargo test --all`
- `uv run pytest python/tests/`

Validation on this branch:
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/` (`115 passed`)

## Intent

Replace the old sequential tend/VSM choreography with a governance model that scans and assesses identity, intelligence, control, and coordination separately, then hands all of those assessments to a single `tend/play-chord` mutation step. The branch also makes the new routing structure first-class in the flow engine and updates the built-in docs, bootstrap script, and wave planning docs so the shipped model, the implementation, and the roadmap all agree.

## Assumptions

- `tend/play-chord` is the right shared execution point for both ordinary tend cycles and the new governance flows.
- Worker pools, `flow` mode, concrete s5/s4/s3/s2/s1 member-wave configs, and concurrent ingest stay out of scope for this branch.
- Structural validation is the right bar here: the parser, builtin registry, and docs should agree even though prompt behavior still depends on live agent execution.

## Key decisions

- Use inline `steps:` on `or` paths so `tend.yaml` can express the tune path directly without keeping a separate `tend-tune` subflow alive.
- Merge draft/apply into `tend/play-chord`, then make `tend/review-chord` explicitly retrospective.
- Split VSM into four governance flows (`govern-identity`, `govern-intelligence`, `govern-control`, `govern-coordination`) instead of keeping the old sequential `vsm.yaml` pipeline.
- Keep the repo-local `.lf/steps/tend/review-chord.md` override aligned with the built-in step so local execution does not silently use the retired pre-approval review behavior.

## Not included

- Worker-pool execution (`workers: N`)
- `flow` replacing `manual`
- Concrete VSM chord member-wave configs
- Concurrent ingest / atomic claiming
