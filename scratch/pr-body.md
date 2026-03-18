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

## Intent

Shrink `lfd`'s container support surface to one honest path. This change removes sandbox as a first-class container executor, makes `mode: container` resolve to Docker only, rejects stale `executor.sandbox` config with a migration error, and updates runtime/docs/compose/tests so they all describe the same Docker-backed deployment story.

## Assumptions

- Docker remains the blessed executor for remote/shared-host container mode.
- Breaking stale `executor.sandbox` config is acceptable as long as the error is explicit and actionable.
- Daytona is still experimental enough that it should stay out of the supported runtime surface for this wave.
- The local macOS UI-test bootstrap failure is environmental noise, not a regression from the one-line bundled-daemon cleanup in this branch.

## Key decisions

- Deleted the sandbox executor path instead of demoting it behind another flag.
- Rejected `executor.sandbox` during config resolution rather than silently ignoring it.
- Tightened the migration check so even a null-valued `executor.sandbox` key fails fast.
- Removed stale sandbox-specific validation/docs/CLI affordances so deploy docs, compose generation, and tests all tell one story.
- Cleaned up a duplicate `LFD_AUTH_MODE` assignment in `BundledDaemonManager` while gating the branch.

## Not included

- Daytona integration
- hidden experimental sandbox fallback paths
- backwards-compatibility shims beyond the explicit migration error
- broader deployment/auth redesign outside the Docker-only container-mode cleanup
