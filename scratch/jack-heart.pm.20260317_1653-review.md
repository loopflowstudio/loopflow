# Branch review: jack-heart.pm.20260317_1653

## What was implemented

- Added the shared PM seam in `rust/loopflow/src/lfd/pm/` and shipped concrete Asana and Linear provider clients behind `PmProvider`.
- Added PM-aware auth plumbing across `lf ops auth`, `lfq auth`, provider storage, HTTP auth routes, and wave config parsing so Asana OAuth and Linear API-key flows work end to end.
- Added PM export/ingest building blocks (`ops/export.rs`, roadmap frontmatter helpers, wave PM config parsing) so waves can map roadmap items to external tracker IDs.
- Landed the attention-queue surface and related API/client model updates that expose items needing human action in lfd and Concerto.
- Gate polish on this pass:
  - Python CLI auth polling now respects provider-reported expiry instead of always using a fixed 180s timeout.
  - README/getting-started now document local Linear auth via `lf ops auth configure linear`.
  - Rust integration tests for config/pr/land now isolate `HOME`, so a developer's personal `~/.lf/config.yaml` no longer breaks test outcomes.

## Key choices

- Keep PM providers as thin transport adapters. `lfd::pm::mod` owns provider-agnostic data structures, while `asana.rs` and `linear.rs` only translate API semantics.
- Treat PM API keys differently from metered model-provider API keys. Asana and Linear auth/status paths avoid misleading pay-per-token messaging.
- Resolve Linear team IDs before client construction. That keeps `LinearClient` focused on GraphQL transport and makes missing-team failures explicit at the call site.
- Fix flaky integration tests by isolating environment-dependent state in tests rather than reshaping production code around local machine config.

## How it fits together

`lfd::pm::mod` defines the shared PM language: provider kind, PM config, roadmap item frontmatter, and the provider trait. `provider_auth`, `lf ops auth`, `lfq auth`, and HTTP auth routes store and surface the credentials needed to instantiate those providers. `ops/export`, wave-config parsing, and roadmap frontmatter then use that seam to connect wave roadmap files to Asana/Linear projects and items, while the attention queue surfaces the human-action side of the broader workflow in the app.

## Risks and bottlenecks

- This is still a large branch: auth plumbing, attention queue work, and PM integrations land together, so reviewers should read it by subsystem.
- Asana still requires markdown/rich-text translation and Linear completion still depends on a team-scoped completed workflow state lookup.
- Swift package validation passes, but the GhosttyKit binary dependency still emits umbrella-header warnings during `swift test`.
- Full PM lifecycle automation is still staged work: this branch establishes the seam and transport, but later waves still need to deepen sync behavior.

## What's not included

- Jira/Notion or any provider beyond Asana and Linear.
- Bidirectional real-time sync, caching, batching, or webhook-driven PM updates.
- Project templates, labels/priority sync, or richer Asana formatting preservation.
- Xcode UI-test validation; this pass covered Swift package tests, not the full macOS UI suite.

## Validation

- `cargo fmt --check` ✅
- `cargo clippy -p loopflow -- -D warnings` ✅
- `cargo test -p loopflow pm::linear` ✅ (9 passed)
- `cargo test -p loopflow --test config_tests` ✅ (22 passed)
- `cargo test -p loopflow --test land_tests` ✅ (7 passed)
- `cargo test -p loopflow --test pr_tests` ✅ (5 passed)
- `uv run pytest python/tests/ -q` ✅ (115 passed)
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅ (16 passed)
- `swift test --package-path swift` ✅ (239 tests passed; existing GhosttyKit header warnings remain)
