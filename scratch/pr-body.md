## Try it!

```bash
# Host side, after enabling HTTPS Certificates in Tailscale admin DNS:
export LFD_AUTH_TOKEN=$(openssl rand -hex 32)
printf '%s\n' "$LFD_AUTH_TOKEN" > ~/.lf/lfd-token && chmod 600 ~/.lf/lfd-token
deploy/tailscale-lfd-host.sh install
deploy/tailscale-lfd-host.sh status
```

```yaml
# Client side: ~/.lf/concerto.yaml
connection:
  host: <host>.<tailnet>.ts.net
  port: 443
  token: "<same bearer token>"
```

Then launch Concerto. Before any saved connection settings, it seeds remote mode from the config and connects over HTTPS. Rotating the token in `~/.lf/concerto.yaml` is picked up on later requests for that same host/port.

Validation run:

```bash
bash -n deploy/tailscale-lfd-host.sh deploy/native-lfd-host.sh
uv run pytest python/tests/ -q
swift test --package-path swift
```

Results: Python `156 passed`; Swift package `339` Swift Testing cases plus `5` XCTest cases passed.

## Intent

Make a Mac mini or other private macOS host usable as a remote Concerto `lfd` server without exposing plain HTTP on the tailnet. Tailscale terminates HTTPS with a real `*.ts.net` certificate, while Concerto can bootstrap and refresh the matching bearer token from local config.

## Assumptions

- Host machines have Tailscale installed and logged in.
- Tailnet HTTPS certificates are enabled before `tailscale-lfd-host.sh install`.
- The remote `lfd` bearer token is still the primary auth boundary; Tailscale limits network reachability, not app-level authorization.
- Concerto config remains single-connection for now.

## Key decisions

- Keep `lfd` loopback-only behind `tailscale serve` instead of adding TLS serving to `lfd`.
- Let CA-trusted certs use system trust, avoiding pinning false positives when Tailscale renews certificates.
- Read config tokens fresh for matching host/port requests so token rotation beats stale Keychain or persisted copies.
- Give dev builds a separate bundle id so local worktree runs do not overwrite installed-app remote settings.

## Not included

- Live tailnet integration test automation.
- Concerto UI screenshot capture; this gate run had no rendering environment.
- Multi-profile remote connection config.
