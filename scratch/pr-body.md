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

## Intent

Collapse the daemon auth story to the two paths the product still wants to support: local session tokens for direct use and studio-managed connection tokens for remote use. This removes the extra pre-shared `ci` bearer-token mode, aligns bundled and container launches around the same session-token semantics, and removes the parallel iOS manual host/token setup path that no longer matches the discovery-first remote flow.

## Assumptions

- Remote deployments should authenticate through studio discovery, so hosts already have valid studio credentials (`~/.lf/credentials.json`) and can register successfully.
- Embedded and bundled launches still need an override path for the local session token, so keeping `auth.token` / `LFD_AUTH_TOKEN` as an internal override is desirable even after deleting `ci` mode.
- No supported iOS workflow still depends on typing a host and bearer token manually.

## Key decisions

- Deleted `AuthMode::Ci` instead of renaming or deprecating it again.
- Reused the session-token file path for override tokens so local and studio loopback auth share one persistence path.
- Switched compose and deploy docs from `LFD_AUTH_PROVIDER` to `LFD_AUTH_MODE`, and documented only the native/local and container/studio shapes.
- Removed the iOS manual connection view instead of keeping a second remote-entry surface beside discovery.

## Not included

- No new self-hosted `team` auth mode; that follow-on work is captured in `wave/trust/06-team-auth.md`.
- No sandbox-policy change; clear-the-deck still has a separate sandbox item.
- No Xcode scheme/workspace fix for the existing `ConcertoUITests` linker issue in this headless environment.

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
