# PM Asana client review

## What was implemented

- Added a shared `lfd::pm` module with provider-agnostic PM types (`PmConfig`, `PmItem`, `PmItemCreate`, `PmItemUpdate`), the `PmProvider` trait, and `RoadmapItemDocument` frontmatter parse/render helpers.
- Added `AsanaClient` in `rust/loopflow/src/lfd/pm/asana.rs` implementing all six `PmProvider` methods against the Asana REST API: create project, list items, create item, update item, complete item, and comment.
- Extended config and auth plumbing so Loopflow can store and report Asana and Linear API keys, parse PM config from `.lf/config.yaml` and wave YAML, and expose the new providers through `lfq auth` flows and onboarding.
- Added tests for Asana pagination, rate-limit retry, sparse updates, error surfacing, config parsing, wave PM config parsing, auth provider parsing, and CLI/auth status rendering.

## Key choices

- Split `pm.rs` into `pm/mod.rs` plus provider files so Asana and future Linear code can share one trait and one data model without mixing transport code into shared types.
- Modeled Asana auth as API-key-backed providers in `provider_auth` instead of forcing them through the CLI/OAuth path. This keeps `lfq auth status` and token storage consistent with the existing provider model.
- Stored Asana descriptions in plain-text `notes` rather than trying to preserve markdown richness through HTML conversion.
- Treated Asana rank as derived response order. `create_item` appends and `update_item` ignores rank-only updates because Asana does not expose a stable numeric rank field.

## How it fits together

`Config` now carries optional `asana` and `linear` blocks, and wave configs can carry an optional `pm` block pointing at a provider/project pair. Provider auth stores API-key credentials for Asana and Linear, while the new `lfd::pm` seam defines the shared document and item model. `AsanaClient` sits behind that seam and translates each trait method into the corresponding Asana HTTP request, including pagination and 429 retry handling.

## Risks and bottlenecks

- `asana.workspace` must be configured before `create_project` can succeed; missing config fails early with a user-facing error.
- Asana ordering is only approximate in v1. Rank is inferred from list order, and rank-only updates are a no-op.
- Notes are sent as plain text, so markdown formatting is intentionally lossy.
- Local macOS UI-test validation still fails in this environment: on March 17, 2026 the `xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` run ended with `ConcertoUITests-Runner ... Early unexpected exit, operation never finished bootstrapping`.

## What's not included

- Linear REST client implementation
- PM import/export steps and run lifecycle sync
- Asana section management, custom fields, dependencies, subtasks, attachments, or webhooks
- Remote task reordering via `insert_before` / `insert_after`

## Validation

- `cargo test -p loopflow pm::asana` ✅ (7 tests passed)
- `cargo fmt --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅ (109 passed)
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅ (16 passed)
- `swift test --package-path swift` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ❌ (`ConcertoUITests-Runner` early unexpected exit before establishing connection)
