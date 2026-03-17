## Try it!

```bash
cargo test -p loopflow lfd::config::tests::auth_mode_yaml_accepts_static_alias_and_canonicalizes_to_ci -- --exact
cargo test -p loopflow lfd::config::tests::legacy_auth_provider_key_is_rejected -- --exact
swift test --package-path swift
rg -n 'ConnectionSetupView|Manual connection' swift
```

What you should see:
- the first Rust test accepts `auth.mode: static` but resolves it to `ci`
- the second Rust test rejects the old `auth.provider` config key
- Swift tests stay green
- ripgrep finds no remaining iOS manual-connection entrypoint

## Validation

```bash
cargo fmt --all --check
cargo clippy -- -D warnings
cargo test --all
swift test --package-path swift
cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -skip-testing:ConcertoUITests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

Additional local note:

```bash
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

The full scheme built and started tests, but `ConcertoUITests-Runner` did not exit in this headless session, so the validation run above skips `ConcertoUITests`.
