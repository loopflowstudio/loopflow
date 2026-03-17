# Gate Review: auth cleanup

## What was implemented

- Renamed lfd auth config from `auth.provider` to `auth.mode` and renamed the Rust auth variant from `Static` to `Ci`.
- Kept `static` as a deprecated alias for `LFD_AUTH_PROVIDER` / YAML `auth.mode` parsing, but reject the legacy `auth.provider` config key.
- Removed the unused iOS manual connection screen and routed iOS settings back through discovery.
- Updated deployment/docs/wave notes to use `ci` terminology and closed the empty growth-cleanup wave item while adding backlog notes for future iOS TestFlight and team-auth work.
- Fixed the production compose override so it no longer shipped the stale `LFD_AUTH_PROVIDER: static` default.

## Key choices

- **Use `mode`, not `provider`, for daemon auth config.** The setting now describes behavior (`local`, `ci`, `studio`) instead of leaking implementation details.
- **Accept `static` only as a mode alias, not as a config shape.** Existing env/YAML values still parse during the rename, but the old `auth.provider` field is rejected to keep one source of truth.
- **Delete the manual iOS flow instead of hiding it.** Discovery via studio is the supported path; leaving a dead manual IP:port form would keep unused code and reviewer confusion around.
- **Keep CI/studio bearer-token plumbing unchanged underneath the rename.** This keeps the behavioral surface narrow while improving naming.

## How it fits together

`AuthMode` is now the canonical daemon auth selector in Rust config loading, daemon setup, and CLI startup checks. The Swift iOS app no longer exposes a second remote-connection path; discovery creates the same `ServerConnection` objects programmatically, while macOS bundled-daemon launchers and deploy docs now emit `ci` auth naming consistently.

## Risks and bottlenecks

- The `static` alias is still accepted, so reviewers should treat this as a transitional naming cleanup rather than a full hard break.
- `auth.provider` is intentionally rejected; any private configs still using that key will need to migrate before upgrading.
- Full `xcodebuild test` with `ConcertoUITests` built and launched locally, but the screenshot UI-test runner stayed live in this headless session. I reran the macOS app test command with `-skip-testing:ConcertoUITests`, which completed successfully. The touched code is not in the screenshot pipeline, but the local UI-test harness remains worth watching.

## What's not included

- No team/self-hosted auth mode implementation; that remains backlog in `wave/trust/06-team-auth.md`.
- No iOS/TestFlight distribution pipeline; that is documented as backlog in `wave/concerto/05-ios-testflight.md`.
- No changes to discovery protocol, connection-token validation, provider auth, or executor behavior.

## Validation

- `cargo fmt --all --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `swift test --package-path swift`
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -skip-testing:ConcertoUITests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`

Additional local note:

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` built and started tests, but the `ConcertoUITests-Runner` did not exit in this headless environment.
