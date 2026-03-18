# Validation — Chord Triggers Review

## Broader validation

```bash
cargo fmt --check
cargo clippy -p loopflow -- -D warnings
orig_home="$HOME"; tmp_home=$(mktemp -d); \
  HOME="$tmp_home" RUSTUP_HOME="$orig_home/.rustup" CARGO_HOME="$orig_home/.cargo" \
  cargo test -p loopflow
swift test --package-path swift
```

Observed locally:
- `cargo fmt --check` ✅
- `cargo clippy -p loopflow -- -D warnings` ✅
- `cargo test -p loopflow` ✅ with isolated `HOME` (local `~/.lf/config.yaml` otherwise affects three pre-existing config tests)
- `swift test --package-path swift` ✅
- `xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ⚠️ local UITest runner bootstrap crash before assertion-level failures (`ConcertoUITests-Runner ... Early unexpected exit, operation never finished bootstrapping`)
