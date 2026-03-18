# Review: Docker-only container mode

## What was implemented

This branch collapses `lfd` container mode to one blessed executor story: `mode: container` now always resolves to Docker.

The runtime no longer carries `ExecutorType::Sandbox`, `AdaptiveContainerExecutor`, or the sandbox executor path. Config resolution now fails fast when `executor.sandbox` appears in `~/.lf/lfd.yaml`, compose generation rejects non-Docker executors for container mode, and deploy/getting-started/daemon docs all describe the same Docker-only path.

Gate polish added two small fixes on top:

- `executor.sandbox` is now rejected even when the stale YAML key is present with a null value, not just `true`/`false`
- `BundledDaemonManager` no longer sets `LFD_AUTH_MODE` twice when starting native bundled `lfd`

## Key choices

- **Delete the branch, don't hide it.** The sandbox executor code and sandbox CI/smoke coverage were removed instead of being kept behind an experimental branch.
- **Fail fast on stale config.** Old `executor.sandbox` configs now produce an explicit migration error instead of being silently tolerated.
- **Treat Docker as the only container contract.** Compose rendering, runtime executor selection, deploy docs, and examples all line up on Docker.
- **Record Daytona as a no-go for this wave.** The branch documents why Daytona is not being promoted into the support surface yet.

## How it fits together

`RawLfdConfig::resolve()` is now the single gate for container-mode executor selection: native mode resolves to local execution, container mode resolves to Docker, and stale sandbox config is rejected before runtime setup. `WaveExecutor` and compose generation both consume that resolved config, so deleting the sandbox branch from config also removes it from runtime and deployment behavior.

The docs mirror that model: getting started, deploy instructions, and the `lfd` reference now all describe the same container setup (`mode: container` + Docker) and the same migration path for old sandbox config.

## Risks and bottlenecks

- **Stale sandbox configs now hard-fail.** That is intentional, but self-hosters with old YAML will need to delete `executor.sandbox` before reinstalling.
- **Swift CI remains environment-sensitive locally.** `swift test --package-path swift` still needs the GhosttyKit XCFramework artifact, and local `xcodebuild test` hit an app/bootstrap failure before UI test execution completed.
- **Scratch/do-when validation was slightly stale.** The design doc used an invalid combined `cargo test` invocation; this gate pass rewrote it to runnable commands.

## What's not included

- Daytona integration
- any hidden or experimental sandbox fallback in mainline config/runtime/docs
- backwards-compatibility shims for `executor.sandbox` beyond the explicit migration error
- broader deployment/auth redesign beyond the Docker-only container decision

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
