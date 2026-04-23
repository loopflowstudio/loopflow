# Flows catalog review

## What was implemented

- Reorganized the built-in flow and step catalog into `build`, `govern`, and `ops`, with bare step names like `scan` instead of `garden/scan`.
- Added the catalog-backed **Flows** tab in Concerto and the `GET /v0/catalog` endpoint in `lfd`.
- Updated repository docs so the README, getting-started guide, daemon reference, Swift README, and shipped design artifacts all describe the same catalog surface.
- Added `swift/ConcertoTests/CatalogTests.swift` so the documented filtered Swift test command exercises real catalog decoding and parent-walk behavior.

## Key choices

- Keep Rust as the source of truth. `lfd` serves the resolved catalog; Swift decodes it and renders it; docs point at the versioned API path instead of describing parallel client-side parsing.
- Document the feature in the user-facing entry points people actually read (`README.md`, `docs/getting-started.md`, `docs/lfd.md`, `swift/README.md`) instead of burying it only in wave notes.
- Add a narrow Swift package test for the new catalog DTO and `directParents` logic rather than only relying on full-suite coverage.

## How it fits together

- Built-in flow and step definitions now live under the agency buckets the UI exposes.
- `lfd` serves the resolved catalog at `/v0/catalog`, merging built-ins with repo overrides.
- `LoopflowCore` decodes that DTO, Concerto renders the Flows tab from it, and the docs tell users and reviewers how to inspect the same structure from the CLI or app.

## Risks and bottlenecks

- Local `xcodebuild test -scheme Concerto` still fails in the UI runner: `ConcertoUITests-Runner` is killed before establishing a connection. Package tests pass; the failure is isolated to the UI harness bootstrap.
- The `/v0/catalog` path now appears in docs, Swift tests, and the app service layer. If the route moves again, these references need to move together.
- Some older archival wave docs outside `wave/flows/` still mention pre-reorg names. I treated those as historical notes, not user-facing references.

## What's not included

- Session-state overlay, `maybe` primitive, search/filter, xor label polish, and iOS layout remain follow-on items under `wave/flows/`.
- No broad cleanup of historical wave docs outside the flows wave.

## Validation

- `cargo test -p loopflow` ✅
- `uv run pytest python/tests/` ✅
- `swift test --package-path swift --filter CatalogTests` ✅
- `swift test --package-path swift` ✅
- `tests/e2e/test_smoke.sh` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ⚠️ UI runner crash before bootstrap
