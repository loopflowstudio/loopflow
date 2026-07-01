# Remote TLS Concerto Gate — Validation

Design and decisions are folded into `wave/desktop/MEMORY.md` (remote connection
patterns). This file keeps only how to re-check the work.

## Try it

```bash
# Host: after enabling HTTPS Certificates in Tailscale admin DNS
export LFD_AUTH_TOKEN=$(openssl rand -hex 32)
printf '%s\n' "$LFD_AUTH_TOKEN" > ~/.lf/lfd-token && chmod 600 ~/.lf/lfd-token
deploy/tailscale-lfd-host.sh install
deploy/tailscale-lfd-host.sh status
```

```yaml
# Client: ~/.lf/concerto.yaml
connection:
  host: <host>.<tailnet>.ts.net
  port: 443
  token: "<same bearer token>"
```

Launch Concerto: before any saved connection settings it seeds remote mode from
the config and connects over HTTPS. Rotating the token in `~/.lf/concerto.yaml`
is picked up on later requests for the same host/port.

## Validation

```bash
bash -n deploy/tailscale-lfd-host.sh deploy/native-lfd-host.sh
uv run pytest python/tests/ -q                 # -> 156 passed
swift test --package-path swift                # -> 339 Swift Testing + 5 XCTest
swift test --package-path swift --filter 'ConcertoConfigTests|ConnectionStoreTests'
```

Wrapper surface checks:
- `deploy/tailscale-lfd-host.sh --help` omits native `serve`
- `deploy/tailscale-lfd-host.sh serve` exits non-zero before touching Tailscale

Concerto UI tests were not run — this gate run had no rendering environment.
