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
