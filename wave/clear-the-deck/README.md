# Clear the Deck

## Vision

Keep Loopflow's post-collapse codebase small and internally coherent. This wave now owns the cleanup passes still crossing Rust CLI/daemon boundaries, Python client contracts, and Concerto's shipped macOS surface. It does not reopen container-executor experiments or add new deployment shapes.

## Strategy

The deployment and auth collapses are now baseline constraints, not active roadmap items. Use this wave to remove the leftover seams those cuts exposed: shared helpers should live in shared layers, `lfd` should have one real command parser, config knobs should expose env overrides consistently, release safety should have isolated tests, and shipped client/app surfaces should match supported platforms instead of leaking demos or stale minimum versions.

Sequence by blast radius. Fix Rust boundary debt and missing release/config coverage first, because those paths shape daily maintenance work. Then clean the Python and Swift surfaces that still advertise or ship the wrong thing. Do not add abstractions to "prepare" for later cleanup. Move code to the layer that already owns the concept, delete duplicates, and tighten tests around the simpler shape.

## Goals

- Shared worktree and execution behavior has one home instead of cross-layer imports or duplicate helpers.
- `lfd` entrypoints and config behave consistently whether invoked from CLI flags, env vars, or release automation.
- Python and Concerto ship only supported surfaces.

## Risks

- Cleanup can sprawl into opportunistic refactors that make the roadmap fuzzy again.
- Moving helpers across layers can fork behavior unless the existing worktree and release tests move with the code.
- Demo-only UI and stale package metadata can linger because they look harmless even though they widen the support surface.

## Metrics

- Cross-layer imports from `lf` into `lfd::executor`: 0
- Duplicated branch-existence helpers in Rust ops/engine code: 0
- Manual subcommand dispatch blocks in `rust/loopflow/src/bin/lfd.rs`: 0
- `lfd` config fields missing env overrides for persisted settings in scope here: 0
- Shipped macOS windows that exist only for demos or tests: 0
- Python minimum-version mismatches between package metadata and lint target: 0
