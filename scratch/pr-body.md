## Try it!

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo test -p loopflow config_
cargo test -p loopflow compose_
cargo test -p loopflow --test land_tests --test pr_tests
cargo test -p loopflow docker_ -- --nocapture
rg -n "executor\.sandbox|ExecutorType::Sandbox|AdaptiveContainerExecutor" rust/loopflow docs deploy docker
```

What you should see:
- all Rust validation passes
- `mode: container` resolves to Docker everywhere
- `rg` only finds intentional migration/history mentions, not live sandbox executor code

Local Swift validation notes:
- `uv run python scripts/check_swift_multiplatform_boundaries.py` passes
- `swift test --package-path swift` still needs a local `GhosttyKit.xcframework`
- local `xcodebuild test` hit a macOS runner/bootstrap failure before UI tests completed
