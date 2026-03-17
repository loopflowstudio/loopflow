## Try it!

```bash
cargo test -p loopflow pm::asana
cargo clippy -- -D warnings

cat >> .lf/config.yaml <<'YAML'
asana:
  workspace: "1234567890"
  default_team: "9876543210"
linear:
  team: "TEAM-ID"
YAML

uv run lfq auth asana
uv run lfq auth linear
uv run lfq auth status
```

What to look for:
- `cargo test -p loopflow pm::asana` runs 7 focused Asana client tests covering pagination, 429 retry handling, sparse updates, and error surfacing.
- `lfq auth status` now shows Asana and Linear alongside the existing providers, and API-key-backed PM providers are not labeled pay-per-token.
- Wave and repo config can now carry PM settings (`pm:` on wave YAML, `asana:` / `linear:` in `.lf/config.yaml`) for the next PM integration steps.

Validation run on this branch:
- `cargo fmt --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅ (109 passed)
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅ (16 passed)
- `swift test --package-path swift` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ❌ with `ConcertoUITests-Runner ... Early unexpected exit, operation never finished bootstrapping`

## Intent

This branch adds the first concrete PM provider implementation for Loopflow: an Asana REST client behind the shared `PmProvider` seam. It also fills in the surrounding config and auth plumbing so Asana/Linear credentials and PM project settings can move through the same surfaces the rest of Loopflow already uses, which unblocks the remaining PM wave work.

## Assumptions

- Teams using Asana will provide a personal access token through `lfq auth asana` and know the target workspace/team IDs to place in `.lf/config.yaml`.
- PM providers authenticate with stored API keys, not OAuth browser flows.
- For v1, Asana task order can be represented by list response order; no remote reorder API is required yet.
- Plain-text task notes are acceptable even though markdown formatting is lossy.

## Key decisions

- Split shared PM types into `rust/loopflow/src/lfd/pm/mod.rs` and kept Asana transport code in `pm/asana.rs` so future providers can plug into the same trait cleanly.
- Added Asana and Linear as API-key providers in `provider_auth`, including status display, onboarding behavior, and environment variable naming, instead of special-casing them outside the auth model.
- Implemented server-directed 429 retry handling and full offset pagination in the Asana client.
- Kept `PmItemUpdate.rank` as advisory for Asana: rank-only updates are ignored and new tasks append.

## Not included

- Linear client implementation
- PM ingest/export/run-lifecycle integration
- Asana sections, dependencies, subtasks, custom fields, attachments, or webhooks
- Numeric/rich remote task reordering
