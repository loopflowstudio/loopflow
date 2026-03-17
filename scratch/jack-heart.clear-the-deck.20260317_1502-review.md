# Gate review — jack-heart.clear-the-deck.20260317_1502

## What was implemented

This branch collapses `lfd` onto the deployment/auth surface the product actually supports:

- deployment is taught as two shapes: `native` and `container`
- auth is taught and enforced as two modes: `local` and `studio`
- container docs/compose default to the blessed `container + studio + Docker` path
- Concerto and `lfd` stop surfacing the old static/manual-token story as a first-class flow

On top of the feature work, this gate pass hardened the Rust test suite so `cargo test --all` no longer depends on the developer's real `~/.lf/config.yaml` or cached gate artifacts.

## Key choices

- **Keep `mode: container` as the real entrypoint.** Docs no longer promise an `lfd install --mode container` flow the CLI does not implement or persist.
- **Collapse auth to `local|studio`.** `auth.provider`, `static`, and the `ci` alias are rejected instead of being carried forward as half-supported compatibility shims.
- **Use one local credential story.** Both native auth and studio's loopback fallback now use the session-token file, so the daemon, compose config, and Concerto all talk about the same credential.
- **Make container/studio the compose default.** `docker/docker-compose.yml` now defaults `LFD_AUTH_MODE` to `studio`, and the prod override stops redundantly setting it.
- **Remove manual iOS token entry.** The mobile UI now routes users back through discovery instead of preserving a connection path the auth model no longer blesses.
- **Stabilize tests by isolating `HOME`.** Gate/PR/config tests now run with a temporary home directory so local machine state cannot flip provider selection or config resolution.

## How it fits together

`LfdConfig::resolve()` now treats `mode` as the deployment shape and `auth.mode` as a tuning choice inside that shape, with container mode auto-promoting `local` without an explicit token to `studio`. `setup_auth()` then materializes either local session-token auth or studio registration from that resolved config, and the docs/compose/Concerto layers now teach the same vocabulary (`mode`, `auth.mode`, session token, Docker executor) instead of separate stories.

## Risks and bottlenecks

- **Config breakage is intentional.** Existing `~/.lf/lfd.yaml` files using `auth.provider`, `static`, or `ci` now fail fast and must be renamed to `auth.mode` with `local` or `studio`.
- **Env/script breakage is intentional.** Callers using `LFD_AUTH_PROVIDER` or `lfd token rotate` need to move to `LFD_AUTH_MODE` and the session-token flow.
- **UI confidence is slightly weaker locally than in CI.** `swift test --package-path swift` passed, but local `xcodebuild test` hit a macOS UI-test runner bootstrap crash after the XCTest suites passed, so final confidence on Concerto UI still leans on CI.

## What's not included

- Team/shared auth beyond today's studio registration flow
- Any product decision about whether `executor.sandbox` stays long-term
- New install-time CLI sugar that persists `mode: container` automatically
- A migration shim for old auth keys/aliases; the branch chooses honesty over backwards compatibility

## Validation

### Deployment-collapse checks

- `grep -nE 'solo|team\\b|ci\\b' docs/lfd.md deploy/README.md docker/docker-compose.yml docs/getting-started.md deploy/docker-compose.prod.yml`
  - only remaining hits are the documented CI endpoint/step references in `docs/lfd.md`, not deployment profile naming
- `wc -l docs/lfd.md deploy/README.md docker/docker-compose.yml`
  - `docs/lfd.md`: 366
  - `deploy/README.md`: 57
  - `docker/docker-compose.yml`: 48
  - total: 471

### Validation run

- `git diff --check` ✅
- `cargo fmt --all` ✅
- `cargo clippy --all-targets -- -D warnings` ✅
- `cargo test --all` ✅
- `cargo test -p loopflow docker_` ✅
- `uv run pytest python/tests/` ✅
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅
- `swift test --package-path swift` ✅
- `cd swift && xcodegen generate && xcodebuild test -derivedDataPath /tmp/loopflow-xcode-dd-20260317-2 -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - XCTest suites passed, then `ConcertoUITests-Runner` exited during bootstrap before establishing a UI-test connection
