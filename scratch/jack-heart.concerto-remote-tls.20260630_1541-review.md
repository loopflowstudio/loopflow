# Remote TLS Concerto Gate

## What was implemented

Added a Tailscale HTTPS front for native `lfd` hosts and taught Concerto how to
bootstrap and refresh a remote bearer token from `~/.lf/concerto.yaml`.

- `deploy/tailscale-lfd-host.sh` wraps `deploy/native-lfd-host.sh`, forces native
  `lfd` to bind `127.0.0.1`, and manages `tailscale serve --https`.
- Concerto config now accepts `connection.token`, seeds the first remote
  connection with it, and rereads it for matching host/port requests.
- CA-trusted TLS chains use system trust instead of certificate pinning, which
  keeps renewed `*.ts.net` certificates from triggering false pin changes.
- Dev app installs use `com.loopflow.concerto.dev` so worktree runs do not share
  installed-app settings.
- macOS UI-test mode avoids bundled daemon startup and remote subscriptions.

## Key choices

TLS stays outside `lfd`. Tailscale already gives the host a real tailnet
certificate, so the native daemon remains an HTTP loopback service and the
wrapper owns secure ingress.

The config token wins over persisted or Keychain tokens only when host and port
match. That makes token rotation immediate without letting a token from one
remote profile leak into another connection.

The wrapper does not expose native `serve`. That command remains an internal
launchd entrypoint, keeping manual usage on install/update/status/health flows.

## How it fits together

On the host, `tailscale-lfd-host.sh install` builds and installs native `lfd`
through the existing launchd script, with `LFD_HTTP_ADDR=127.0.0.1:<port>`, then
starts `tailscale serve --https=<port> http://127.0.0.1:<lfd-port>`.

On the client, Concerto loads a single `connection` block from
`~/.lf/concerto.yaml`. If no saved settings exist and the host is non-loopback,
it seeds remote mode. Later requests call `ConnectionStore.token(for:)`, which
reloads the config and prefers its token for the same host/port.

## Risks and bottlenecks

- Live tailnet behavior is not covered in CI; validation is script syntax/help,
  unit coverage, and manual host setup commands.
- The config parser is intentionally small and only supports the documented
  top-level shape. Multi-profile YAML is out of scope.
- `tailscale serve` assumes HTTPS certificates are enabled in the tailnet before
  install.

## What's not included

- Multi-profile remote connection config.
- TLS serving inside `lfd`.
- Live Tailscale integration tests.
- Concerto UI screenshot capture; this run had no rendering environment.

## Validation

```bash
bash -n deploy/tailscale-lfd-host.sh deploy/native-lfd-host.sh
uv run pytest python/tests/ -q
swift test --package-path swift
```

Results:

- Shell syntax check passed.
- Python: `156 passed in 4.90s`.
- Swift package: `339` Swift Testing cases and `5` XCTest cases passed.

Wrapper surface checks:

- `deploy/tailscale-lfd-host.sh --help` includes `serve-off` and
  `TS_HTTPS_PORT=443`.
- `deploy/tailscale-lfd-host.sh --help` omits native `serve`.
- `deploy/tailscale-lfd-host.sh serve` exits non-zero with usage before trying
  to resolve or run Tailscale.
