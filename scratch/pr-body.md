## Try it!

```bash
rg -n "conversation|Conversation|conversations|UsageAnalytics|AnalyticsDashboard|CostEstimator" -S
rg -n "lfq usage|scripts/test_session.py|/v0/conversations|conversations/" README.md TESTING.md docs python rust swift scripts tests wave -S
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
uv run pytest python/tests/
swift test --package-path swift
uv run python scripts/check_swift_multiplatform_boundaries.py
cd website && uv run python dev.py test
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
uv run pytest tests/regression/ -v
```

Expected: no live conversation API/client roots or stale removed-command docs remain; Rust, Python, Swift package, website, regression, and e2e smoke checks pass.

## Intent

Remove the dormant conversations subsystem so loopflow has one live runtime vocabulary: waves, runs, and sessions. This clears stale API/UI/store surface before the Session Record work and adds reduce wave state so future simplification work has a durable home.

## Assumptions

External compatibility for `/v0/conversations/*` is not required. Usage analytics were coupled to conversation events, so removing the dashboard is preferable to preserving a dead data path.

## Key decisions

- Deleted conversation routes, DTOs, store methods, Python/Swift clients, docs, and tests instead of leaving shims.
- Kept session create/get/stop/attach as the live control surface.
- Added `docs/architecture.md` to make the post-removal system shape easy to review.
- Marked the reduce queue item done with the gate validation record.

## Not included

No replacement transcript subsystem, no Session Record aggregate, and no rebuilt usage/cost dashboard.

## Validation notes

`cargo test -p loopflow docker_ -- --nocapture` passed, with two Docker-runtime cases skipped because `/var/run/docker.sock` was not available locally. `xcodebuild build -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` passed. The bounded local `xcodebuild test ... -skip-testing:ConcertoUITests` run timed out after 300 seconds while building test targets, before any test result; `swift test --package-path swift` and the Swift boundary guard passed.
