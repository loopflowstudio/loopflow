# Remote TLS Concerto Gate Review

## What Was Implemented

Added a Tailscale HTTPS front for native macOS `lfd` hosts, and taught Concerto to consume that remote endpoint cleanly:

- `deploy/tailscale-lfd-host.sh` installs/updates native `lfd` on loopback and manages `tailscale serve` as the HTTPS ingress.
- Concerto dev builds now use `com.loopflow.concerto.dev`, keeping worktree dev settings and installed-app settings separate.
- `~/.lf/concerto.yaml` remote connections can include a bearer token; matching requests read that token fresh so rotation does not require Settings re-entry.
- CA-trusted TLS certificates, including `*.ts.net`, use system trust instead of certificate pinning.
- macOS UI-test mode avoids starting bundled daemons or remote subscriptions.

## Key Choices

- Kept TLS termination outside `lfd`. The native daemon stays on `127.0.0.1`, and Tailscale owns cert issuance/renewal.
- Config-file tokens beat static connection tokens and Keychain tokens only when host and port match. This prevents a token in one remote profile from leaking to another connection.
- The Tailscale wrapper does not expose native `serve`; that command remains an internal launchd entrypoint on `native-lfd-host.sh`.
- Dev identity changes happen while assembling `Concerto Dev.app`, leaving source `Info.plist` unchanged.

## How It Fits Together

The host path is `native-lfd-host.sh` for launchd lifecycle plus `tailscale-lfd-host.sh` for HTTPS ingress. Concerto seeds or refreshes remote credentials from `~/.lf/concerto.yaml`, then normal connection code asks `ConnectionStore.token(for:)` for the active token before HTTP/WebSocket calls.

## Risks And Bottlenecks

- `tailscale serve` must be installed, logged in, and have tailnet HTTPS certificates enabled.
- This does not test a live tailnet host in CI; local validation covers script syntax/help behavior and documented command surfaces.
- Token refresh is file-read based. It favors correctness and immediate rotation over caching.

## What's Not Included

- No bundled TLS support inside `lfd`.
- No multi-profile Concerto config schema; the current config remains one remote connection plus optional container settings.
- No UI screenshot capture in this gate pass because this run has no rendering environment.

## Validation

- `bash -n deploy/tailscale-lfd-host.sh deploy/native-lfd-host.sh`
- `uv run pytest python/tests/test_release_automation.py -q` -> 8 passed
- `swift test --package-path swift --filter 'ConcertoConfigTests|ConnectionStoreTests'` -> 17 Swift Testing cases passed
- `uv run pytest python/tests/ -q` -> 156 passed
- `swift test --package-path swift` -> 339 Swift Testing cases passed, plus 5 XCTest cases
- Wrapper smoke: `deploy/tailscale-lfd-host.sh --help` omits native `serve`; `deploy/tailscale-lfd-host.sh serve` exits non-zero before touching Tailscale

Concerto UI tests were not run; the session explicitly has no rendering environment.
