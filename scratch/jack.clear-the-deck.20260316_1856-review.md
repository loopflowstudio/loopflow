# Review: jack.clear-the-deck.20260316_1856

## What was implemented

- Removed the public `ci` auth mode from `lfd`; the daemon now exposes only `local` and `studio` auth modes.
- Reused the session-token path for both modes so `auth.token` / `LFD_AUTH_TOKEN` act as an optional local session-token override instead of a separate remote bearer-token mode.
- Updated compose generation, deploy docs, daemon docs, and bundled-daemon launch paths to use `LFD_AUTH_MODE` and the smaller local/studio deployment story.
- Deleted the iOS manual host/token connection screen so iPhone setup now flows through discovery only.
- Trimmed the clear-the-deck and trust wave docs to match the smaller surface: deployment collapse and sandbox pruning stay in this wave; future self-hosted team auth moved to `wave/trust/06-team-auth.md`.

## Key choices

- **Delete the branch, don’t rename it again.** Instead of carrying `static -> ci -> team` churn forward, the branch removes the pre-shared remote bearer-token mode entirely and leaves only the paths the product still wants to support.
- **Keep `LFD_AUTH_TOKEN`, but demote it to an override.** Embedded and bundled launches still need a way to preseed the local session token, so the env/config key stays as an override instead of a documented remote-auth mode.
- **Share one session-token write path.** `setup_auth()` now writes overrides through the same session-token file handling used for generated tokens, which keeps local and studio loopback behavior aligned.
- **Discovery is the only iOS remote entrypoint.** The manual host/token screen was removed rather than partially maintained beside studio discovery.

## How it fits together

`AuthMode` now models the supported daemon entrypoints directly: `Local` writes or reuses a session token, and `Studio` does the same local-token setup before registering for remote connection-token distribution. Container docs and compose templates now point at studio auth by default, while bundled and embedded launches can still inject a local token override through `LFD_AUTH_TOKEN`. On the client side, iOS no longer exposes a separate manual token flow, so remote access stays aligned with discovery and provider auth.

## Risks and bottlenecks

- Remote self-hosted deployments now assume working studio credentials on the host (`~/.lf/credentials.json` plus successful registration).
- Any external scripts still setting `auth.mode: ci` or `LFD_AUTH_PROVIDER` will fail fast and need migration.
- Xcode scheme validation remains noisy in this headless environment: `xcodebuild` still attempts to link `ConcertoUITests` even when skipping them and fails with `can't write output file .../ConcertoUITests`.

## What's not included

- No self-hosted `team` auth mode; that follow-on work is captured in `wave/trust/06-team-auth.md`.
- No sandbox removal or rollout change; clear-the-deck still has a separate sandbox decision item.
- No replacement for direct host/token iOS setup; the product direction here is discovery-only, not a second remote-entry UI.

## Validation

### Done-when checks covered

- Public deployment surface reduced to two documented shapes: native/local and container/studio.
- Shipped docs and compose examples no longer describe `ci` / static-token daemon auth.
- Local and remote smoke coverage stayed green.

### Measurable outcomes

- Documented public auth modes in shipped daemon docs: **2** (`local`, `studio`), down from **3**.
- Dedicated iOS manual connection screens: **0**, down from **1**.
- Matches for removed shipped terminology in `docs deploy swift rust/loopflow/src/lfd`: **0** for `LFD_AUTH_PROVIDER`, `static auth token`, and `CI auth token`; **0** for `auth.mode=ci` outside the rejection test fixture.

### Commands run

- `cargo fmt --all --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- `cargo test -p loopflow docker_`
- `cargo test -p loopflow sandbox`
- `cargo test -p loopflow lfd::config::tests::auth_mode_yaml_rejects_removed_ci_alias -- --exact`
- `cargo test -p loopflow lfd::config::tests::legacy_auth_provider_key_is_rejected -- --exact`
- `cargo test -p loopflow session_token::tests::write_persists_existing_token -- --exact`
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -skip-testing:ConcertoUITests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` *(fails in this headless session because Xcode still links `ConcertoUITests` and errors `can't write output file .../ConcertoUITests`)*
