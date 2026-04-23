## Try it!

```bash
cargo test -p loopflow
uv run pytest python/tests/
swift test --package-path swift --filter CatalogTests
swift test --package-path swift
tests/e2e/test_smoke.sh

lfd serve
curl -s "http://127.0.0.1:2486/v0/catalog?repo=$(pwd)" | jq '.result.flows[] | {name, category, source}'
```

Then open Concerto, switch to **Flows**, expand `build` → `build`, and click `gate`. The left pane should show the nested flow structure grouped under **Build / Govern / Ops**; the right pane should show every parent flow that reaches the selected item.

Validation run on this branch:
- ✅ `cargo test -p loopflow`
- ✅ `uv run pytest python/tests/`
- ✅ `swift test --package-path swift --filter CatalogTests`
- ✅ `swift test --package-path swift`
- ✅ `tests/e2e/test_smoke.sh`
- ⚠️ `xcodebuild test -scheme Concerto` currently crashes `ConcertoUITests-Runner` before the UI harness finishes bootstrapping

## Intent

Make Loopflow's flow system legible again. This ships the three-bucket built-in taxonomy (`build`, `govern`, `ops`), exposes the resolved catalog from `lfd`, renders it in Concerto's new Flows tab, and updates the repo docs so users can find and verify the feature without grepping the codebase.

## Assumptions

- The catalog should stay behind the versioned API path `GET /v0/catalog`.
- Rust remains the only parser for built-in and repo-local flow structure; Swift consumes the DTO and does the upward “used by” walk client-side.
- Historical wave docs outside `wave/flows/` are archival unless they are explicitly being refreshed in this branch.

## Key decisions

- Flatten built-in flow and step names to bare names while keeping category information in the catalog metadata.
- Document the feature in the main user entry points (`README`, getting-started, daemon reference, Swift README) instead of leaving it only in design notes.
- Add focused `CatalogTests` so the documented filtered Swift command is real and checks both DTO round-tripping and direct-parent discovery.

## Not included

- Session-state overlay on top of the static catalog
- `maybe` primitive migration and xor cleanup
- iOS/search/filter polish for the Flows surface
- Broad cleanup of older archival wave docs
