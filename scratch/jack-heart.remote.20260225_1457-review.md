# Review: Remote EC2 Dogfood Deploy + Smoke Harness

## What was implemented

Production deployment path for `lfd` on EC2 behind Caddy TLS, plus a single remote smoke script (`scripts/test_remote_smoke.py`) that validates the full API surface over the proxy.

Concrete deliverables:

- **`deploy/`** — Caddyfile (env-driven domain/TLS, websocket routing, SSE flush), docker-compose prod overlay, and operator README
- **`scripts/test_remote_smoke.py`** — 7 scenarios: health, auth rejection, wave CRUD, SSE session events, websocket handshake, wave run + log streaming, websocket reconnect snapshot
- **`scripts/lib/api_harness.py`** — extended with TLS verification controls (`verify` param) and `stream()` method
- **`python/tests/test_remote_smoke_script.py`** — unit tests for repo resolution and TLS arg validation
- **Wave/doc updates** — EC2 dogfood marked shipped in wave README; learnings carried to Mac Mini item; TESTING.md updated

## Key choices

| Decision | Why | Alternatives rejected |
|----------|-----|----------------------|
| Single end-to-end script vs fragmented curl checks | One command to validate a deploy. Easier to maintain and extend for Mac Mini parity | Separate scripts per scenario — harder to maintain shared TLS/auth setup |
| Three TLS modes (default, `--ca-cert`, `--insecure`) | Covers public ACME, internal CA, and bootstrap/dev situations | Hardcoding trust assumptions — too fragile for varying deploy contexts |
| Require `--repo` on fresh hosts | `/v0/repos` returns empty on first run; failing with a clear message is better than a confusing 404 deep in a scenario | Auto-creating a repo — too presumptuous about remote state |
| `ApiClient.stream()` consolidation | Eliminated the separate `httpx.Client` for streaming — one client handles both regular and streaming requests | Keeping two clients — unnecessary duplication |
| `websockets` as dev dependency with runtime guard | Script users get a clear error if the dep is missing, but the import doesn't break pytest collection | Making it a hard dependency — it's only needed for the smoke script |

## How it fits together

```
laptop                          EC2 host
──────                          ────────
test_remote_smoke.py  ──HTTPS──▶  Caddy (TLS)  ──HTTP──▶  lfd:2486
  ├─ ApiClient (httpx)                                       ├─ /health
  ├─ WebSocketClient (websockets)                            ├─ /v0/waves
  └─ ScenarioRunner (harness)                                ├─ /v0/sessions
                                                             └─ /ws
```

The smoke script uses `ApiClient` for all HTTP (including SSE streaming) and `WebSocketClient` for WS scenarios. `ScenarioRunner` executes each scenario, catches failures, and prints a PASS/FAIL summary. Cleanup of created waves happens in a `finally` block.

Caddy handles TLS termination, websocket upgrade routing, and SSE-friendly `flush_interval -1`. The prod compose overlay layers Caddy + static token auth on top of the existing `docker/docker-compose.yml`.

## Risks and bottlenecks

- **SSE latency through Caddy not load-tested.** Smoke validates event receipt but not sustained throughput. Monitor during Mac Mini parity work.
- **Session scenario depends on remote harness.** If `claude` (default `--session-harness`) isn't configured on the remote host, the SSE scenario fails. This is documented but could surprise first-time operators.
- **No CI for remote smoke.** The script runs manually against a live host. CI automation against external hosts is explicitly out of scope — it would need a managed test instance.
- **websockets library version sensitivity.** The script introspects `websockets.connect` signature to handle `additional_headers` vs `extra_headers` across versions. This is fragile if the library changes again.

## What's not included

- Host provisioning automation (Terraform, cloud-init)
- Studio/JWT auth rollout — this branch uses static token only
- `/v0/repos` API changes — the script works around the empty-repo case with `--repo`
- Mac Mini parity — that's wave item 02, which consumes this branch's smoke script
- Concerto remote UX validation — manual only (documented in deploy/README.md)
