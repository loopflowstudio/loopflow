## Try it!

```bash
cargo test -p loopflow lfd::config::tests::auth_mode_yaml_rejects_removed_ci_alias -- --exact
cargo test -p loopflow lfd::config::tests::legacy_auth_provider_key_is_rejected -- --exact
cargo test -p loopflow session_token::tests::write_persists_existing_token -- --exact
swift test --package-path swift
rg -n 'LFD_AUTH_PROVIDER|static auth token|CI auth token' docs deploy swift rust/loopflow/src/lfd || true
rg -n 'auth\.mode=ci' docs deploy swift rust/loopflow/src/lfd --glob '!config.rs' || true
```

What you should see:
- the first Rust test rejects `auth.mode: ci`
- the second Rust test rejects the old `auth.provider` config key
- the session-token test proves explicit overrides still persist through the shared session-token writer
- Swift package tests stay green
- both ripgrep commands print nothing, showing the shipped docs and daemon surface no longer describe `ci` / static-token auth

## Validation

- `cargo fmt --all --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅
- `cargo test -p loopflow docker_` ✅
- `cargo test -p loopflow sandbox` ✅
- `swift test --package-path swift` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -skip-testing:ConcertoUITests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ⚠️ still fails in this headless session because Xcode links `ConcertoUITests` anyway and errors `can't write output file .../ConcertoUITests`
