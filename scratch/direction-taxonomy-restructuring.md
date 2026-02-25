# Direction Taxonomy Restructuring

## Status

Completed.

This is the canonical summary for this work item. It replaces earlier planning and review notes.

## Goal

Replace role-style directions with composable quality-focused direction groups, and make `-d <group>` work consistently across prompt context gathering and fork execution.

## Final taxonomy

```
rust/loopflow/src/engine/builtins/directions/
  infra/
    security.md
    performance.md
    reliability.md
    observability.md
  ux/
    visibility.md
    feedback.md
    consistency.md
    affordance.md
    error-prevention.md
    accessibility.md
    dynamics.md
    aesthetics.md
  values/
    clarity.md
    simplicity.md
    craft.md
    flow.md
    scale.md
  ceo.md
```

Removed: `roles/infra-engineer.md`, `roles/designer.md`, `roles/product-engineer.md`, and `roles/` itself.  
Moved: `roles/ceo.md` to top-level `ceo.md`.

## Implemented

- Added build-time builtin direction group generation (`BUILTIN_DIRECTION_GROUPS`) and public lookup APIs in `engine::builtins`.
- Added direction group expansion before direction loading, with:
  - user-defined group directories in `.lf/directions/<group>/` taking precedence over builtin groups,
  - stable dedupe behavior preserving order.
- Applied group expansion in:
  - prompt context assembly (`gather_context`),
  - CLI fork planning path,
  - daemon wave fork planning path.
- Updated user direction resolution to find nested `.lf/directions/*/*.md` files.
- Updated plan fork flows (`wave-reduce`, `wave-polish`, `wave-expand`) to fork across `infra`, `ux`, and `ceo`.
- Updated quality-language in builtin steps:
  - `interactive/review.md`
  - `interactive/review-design.md`
  - `code/gate.md`
- Added/updated tests for:
  - group expansion behavior (builtin, user, mixed, dedupe),
  - nested direction lookup,
  - fork flow direction parsing,
  - golden prompt parity with grouped directions.
- Updated docs/examples/scripts to the new taxonomy.

## Key decisions

- Expand groups before loading concrete direction files, keeping downstream logic unchanged.
- Resolve user groups before builtin groups when names collide.
- Do not maintain compatibility aliases for removed role names.

## Risks and migration notes

- Any automation still passing removed names (`infra-engineer`, `designer`, `product-engineer`) must migrate.
- Leaf-name collisions across groups can still cause ambiguity if duplicate filenames are introduced.
- `cargo test --all` still includes Docker-socket-dependent failures in environments without `/var/run/docker.sock`.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow --test flow_tests --test context_tests --test discovery_tests --test golden_prompt`
- `cargo test -p loopflow engine::flow::tests::`
- `cargo test --all` *(fails only on Docker socket-dependent tests in this environment)*
- `uv run pytest python/tests/ -q`
- `swift test --package-path swift`
