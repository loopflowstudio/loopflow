# Attention queue validation

## Try it

- `cargo fmt --check && cargo clippy -- -D warnings && cargo test --all`
- `swift test --package-path swift`
- Open Concerto without selecting a wave. The repo window should land on the attention queue instead of the old empty detail state.
- Exercise a code review item and verify the detail view exposes `Ship`.
- Exercise a step failure item and verify the detail view exposes `Retry`.

## Additional check

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
- On this machine on March 17, 2026, the scheme's unit tests passed but `ConcertoUITests-Runner` exited before bootstrapping twice; rerun once that local UI-test issue is understood.
