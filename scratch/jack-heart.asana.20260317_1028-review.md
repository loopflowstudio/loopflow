# Review: PM trait, config, and provider auth scaffolding

## What was implemented

- Added a provider-agnostic PM foundation in Rust: `PmProvider`, PM item/config types, and roadmap frontmatter parsing/rendering for `pm_id`.
- Extended wave and global config parsing to understand `pm.provider`, `pm.project`, `.lf/config.yaml` `asana` settings, and `.lf/config.yaml` `linear` settings.
- Added Asana and Linear to provider auth in both Rust and Python, including `lfq auth asana`, `lfq auth linear`, provider status rendering, and token storage via the existing API-key path.
- Updated onboarding/auth naming helpers so provider labels and env-var lookups live in one place.
- Added roadmap docs for the full PM wave and README/getting-started updates for the new auth commands.

## Key choices

- Put PM primitives in `rust/loopflow/src/lfd/pm.rs` now, before any Asana or Linear client code. That keeps the provider boundary explicit and lets later waves build against stable types.
- Kept PM config optional everywhere. Waves without a `pm` block still deserialize exactly as before.
- Reused the existing provider-token storage path for Asana and Linear instead of inventing PM-specific credential plumbing.
- Split API-key billing semantics from API-key storage semantics during gate: Claude/Codex/OpenCode Zen still show metered API-key copy, while Asana/Linear now show neutral API-key copy so reviewers and users are not misled.

## How it fits together

The new `pm` module defines the shared PM vocabulary: provider kind, project config, item shapes, and roadmap frontmatter parsing. Wave YAML parsing (`wave_config.rs`) and global config parsing (`engine/config.rs`) consume those types, while provider auth extends the existing auth service and CLI so Asana/Linear credentials can be stored and surfaced through the same `/auth` endpoints and `lfq auth` UX as other providers.

## Risks and bottlenecks

- This branch only establishes the abstraction and credential/config plumbing; it does not prove the Asana REST or Linear GraphQL mappings yet.
- PM credentials are stored through the generic API-key path, so any future PM-specific metadata requirements will need to extend that model carefully.
- `RoadmapItemDocument` only handles frontmatter wrapped in the repo's current `--- ... ---` format; if roadmap files drift from that convention, later import/export work will need to normalize them.

## What's not included

- Asana API client
- Linear API client
- `lf ops asana/linear init|link|status`
- `import-pm` / `export-pm`
- ingest auto-refresh from PM
- run lifecycle comments/completion sync

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- Added a focused Python regression test that ensures Asana API-key auth status does not claim pay-per-token billing.
