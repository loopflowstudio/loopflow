# Wave Crons — Validation

## Try it

```bash
cargo test --all
uv run pytest python/tests/
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests
```

## Done when

- `crons` field on wave config YAML parsed and stored in `wave_crons` table
- `WaveCron` type exists in Rust, Python, and Swift models
- `lfd` cron poller fires supplementary flows on schedule independently of worker pool
- Each cron entry tracks its own `last_triggered_at`
- `workers: 0` works for waves with crons
- `GET /waves/{id}` returns crons in the response
- `GET /waves/{id}/crons` returns dedicated cron list
