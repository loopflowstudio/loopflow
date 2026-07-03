## Try it!

```bash
rg -n "Conversation(EventEnvelope|Store|Filters)|UsageAnalytics|AnalyticsDashboard|CostEstimator|sendConversationInput|streamConversationEvents|usage_summary|/v0/conversations|lfq usage|scripts/test_session.py" README.md TESTING.md docs python rust swift scripts tests -S
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
docker version
cargo test -p loopflow docker_ -- --nocapture
cd swift && xcodegen generate
xcodebuild build -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -skip-testing:ConcertoUITests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

Expected: no live conversation API/client roots or stale removed-command docs remain; Rust, Python, Swift package, website, regression, e2e, Docker-marked, and Concerto Xcode checks pass.

## Intent

Remove the dormant conversations subsystem so loopflow has one live runtime vocabulary: waves, runs, and sessions. This clears stale API/UI/store surface before the Session Record work and adds reduce wave state so future simplification work has a durable home.

## Assumptions

External compatibility for `/v0/conversations/*` is not required. Usage analytics were coupled to conversation events, so removing the dashboard is preferable to preserving a dead data path.

## Key decisions

- Deleted conversation routes, DTOs, store methods, Python/Swift clients, docs, and tests instead of leaving shims.
- Kept session create/get/stop/attach as the live control surface.
- Removed stale Swift event reducers and parsing helpers that only fed the deleted conversation event stream.
- Added `docs/architecture.md` to make the post-removal system shape easy to review.
- Marked the reduce queue item done with the gate validation record.

## Not included

No replacement transcript subsystem, no Session Record aggregate, and no rebuilt usage/cost dashboard.

## Validation notes

Gate reran at `789f6e71b72b17c6443de334b86b26c18fe3bbb9`; all commands above passed. `cargo test -p loopflow docker_ -- --nocapture` passed, with two Docker-runtime cases skipped because `/var/run/docker.sock` was not available locally. `xcodebuild build ...` passed. `xcodebuild test ... -skip-testing:ConcertoUITests` passed with 310 tests.
