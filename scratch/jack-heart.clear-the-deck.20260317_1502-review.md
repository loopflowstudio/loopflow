# Review: Docker-only container mode

## Validation

### Passed

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `cargo test -p loopflow config_`
- `cargo test -p loopflow compose_`
- `cargo test -p loopflow --test land_tests --test pr_tests`
- `cargo test -p loopflow docker_ -- --nocapture`
- `uv run python scripts/check_swift_multiplatform_boundaries.py`
- `xcodegen generate`
- `rg -n "executor\.sandbox|ExecutorType::Sandbox|AdaptiveContainerExecutor" rust/loopflow docs deploy docker`

### Not green locally

- `swift test --package-path swift`
  - failed with `XCFramework Info.plist not found` for local `GhosttyKit.xcframework`
- `xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - built and ran unit/app test work, but the macOS test session exited with code 65 before the runner finished bootstrapping
  - xcresult paths captured locally:
    - `/Users/jack/Library/Developer/Xcode/DerivedData/LoopflowSwift-hcbbjybaeqmsswfntbeceolujsgh/Logs/Test/Test-Concerto-2026.03.17_21-32-14--0700.xcresult`
    - `/Users/jack/Library/Developer/Xcode/DerivedData/LoopflowSwift-hcbbjybaeqmsswfntbeceolujsgh/Logs/Test/Test-Concerto-2026.03.17_21-33-34--0700.xcresult`
