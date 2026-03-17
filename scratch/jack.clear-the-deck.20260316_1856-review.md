# Review: jack.clear-the-deck.20260316_1856

## Validation

### Done-when checks covered

- Public deployment surface reduced to two documented auth shapes: local and studio.
- Shipped docs and compose examples no longer describe `ci` / static-token daemon auth.
- Local and remote smoke coverage stayed green.

### Measurable outcomes

- Documented public auth modes in shipped daemon docs: **2** (`local`, `studio`), down from **3**.
- Dedicated iOS manual connection screens: **0**, down from **1**.
- Matches for removed shipped terminology in `docs deploy swift rust/loopflow/src/lfd`: **0** for `LFD_AUTH_PROVIDER`, `static auth token`, and `CI auth token`; **0** for `auth.mode=ci` outside the rejection test fixture.

### Commands run

- `cargo fmt --all --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- `cargo test -p loopflow docker_`
- `cargo test -p loopflow sandbox`
- `cargo test -p loopflow lfd::config::tests::auth_mode_yaml_rejects_removed_ci_alias -- --exact`
- `cargo test -p loopflow lfd::config::tests::legacy_auth_provider_key_is_rejected -- --exact`
- `cargo test -p loopflow session_token::tests::write_persists_existing_token -- --exact`
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -skip-testing:ConcertoUITests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` *(fails in this headless session because Xcode still links `ConcertoUITests` and errors `can't write output file .../ConcertoUITests`)*
