# Branch validation: wave crons + concurrent ingest coordination

## Passed

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`

## Try it

Create a wave config with supplemental crons and inspect the returned wave JSON/UI payload for `crons`, then run concurrent ingest tests:

```bash
cargo test ops::ingest::tests::concurrent_ingest_picks_different_items
```

## Needs CI confirmation

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - Local result after wiping DerivedData: `ConcertoUITests-Runner` exits before bootstrapping (`signal kill`). Swift package tests pass.
