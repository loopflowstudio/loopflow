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

Validated locally:
- `cargo fmt --all --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `swift test --package-path swift`
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -skip-testing:ConcertoUITests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`

I also ran the full macOS scheme test once via `xcodegen generate && xcodebuild test ...`; build + non-UI tests completed, but the screenshot UI-test runner stayed live in this headless session, so I reran the macOS validation with `-skip-testing:ConcertoUITests`.

## Intent

Clean up the daemon auth surface so the configuration names describe actual modes (`local`, `ci`, `studio`) instead of implementation details, and remove the unused iOS manual IP:port connection flow now that discovery via studio is the supported path.

## Assumptions

- Existing users can migrate from `auth.provider` to `auth.mode` without needing a config-shape compatibility layer.
- Keeping `static` as a deprecated value alias is enough transition support for env vars and YAML values already in circulation.
- Discovery remains the only supported remote-connect path on iOS; no product flow depends on manual host/token entry.
- The local `ConcertoUITests` hang is a harness issue in headless macOS, not a regression in this auth cleanup.

## Key decisions

- Canonicalized the Rust enum/config terminology to `AuthMode` / `Ci`, but kept the external env var name `LFD_AUTH_PROVIDER` for now so deployment surfaces do not churn more than necessary.
- Rejected the legacy `auth.provider` YAML key outright instead of silently supporting both shapes.
- Deleted `ConnectionSetupView.swift` rather than leaving it orphaned or hidden behind a link.
- Fixed the production deploy override to default to `ci` auth too, so docs and shipping compose config match.

## Not included

- Team/self-hosted auth mode
- TestFlight/iOS distribution work
- Discovery/auth protocol changes beyond the config rename and terminology cleanup
