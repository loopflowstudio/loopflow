# Open Questions

Questions that emerged during lfd-primary implementation. Need resolution before or during next iteration.

## Architecture

- **RunWave overrides**: `flow`, `direction`, `area` overrides currently persist to `Wave`. Design says wave config shouldn't change. Should we add override fields to `WaveRun` instead?

## Scheduler slots

- **Watch/cron bypass slots**: Loop ticker acquires scheduler slots before creating WaveRun, but watch/cron triggers start runs without slot checks. Should they also acquire/release scheduler slots?
- **Interactive resumes skip slot checks**: ConnectWave/EndAgent resumes call executor directly without acquiring scheduler slots. Should interactive resumes also acquire/release slots?

## Deferred features

- **Choose/Fork selection**: `ForkSelect::One` picks first option deterministically (no LLM). `ForkSelect::Prompt` also picks first. When should we wire a choice agent?
- **Fork retry tracking**: Design has `fork_attempts` placeholder but it's not implemented. Add when we hit transient failures in practice.

## WaveService Protocol

- Should `/wave-runs` move to a `/v1/` endpoint or keep legacy-style `ok/result` responses long-term?
- Should `WaveServiceFactory` select contexts from config (grpc/remote) instead of always returning `LocalWaveService`?

## Concerto Wave Phase Status

**Observation**: The Phase 1 ordered set in `roadmap/concerto/README.md` references items that don't exist:
- `20260131-02-history-and-recency.md`
- `20260131-03-waiting-state-actionable.md`
- `20260131-04-running-state-progress-and-connect.md`
- `20260131-05-empty-state-creates-and-teaches.md`
- `20260131-06-quick-experiment-path.md`

Recent commits suggest items 04 and 06 have shipped. Item 01 is noted as complete in the README.

**Question**: Is Phase 1 complete? Should we:
1. Move to Phase 2 (Remote access foundation)?
2. Update the README to reflect Phase 1 completion?
3. Create the missing Phase 1 items if they're not actually done?

The remaining items in `roadmap/concerto/` are all Phase 2 or 3.

## Auth Implementation

- Refresh endpoint response: assumed /auth/refresh returns JSON {token|jwt} or raw token string; confirm actual contract.
- Package supports macOS 15 only; design doc targets macOS 14+/iOS 17+. If iOS support is required, update Swift package platforms and add iOS app target.
- RemoteWaveService not present in repo; TokenProvider added but not wired into any remote service yet.

## lf ops (Rust)

1. **Rebase assistant prompt**: Rust ops uses a custom inline prompt instead of a dedicated `rebase` step file. There is no Rust builtin `rebase` step today. Should we add builtin ops steps to `loopflow-engine` and switch to them?
2. **Next branch naming**: `lf ops next` still uses `next-<timestamp>` instead of wave-based branch naming (no wave metadata update in Rust yet). Should we wire it to wave naming once the wave module is ported?
3. **Land local strategy**: `lf ops land --local` uses squash merge via `git::land` and skips PR-related steps. Confirm if we need a local merge option or to support `--strategy` explicitly.
4. **Lint fixer**: lint retry uses the built-in `lint` step via prompt context, not `lf lint -b` subprocess. Is that preferred for parity?

## lfd Registration (Phase 2) — RESOLVED

**Decision**: Relay through loopflow.studio is the primary remote path. Direct connections are a power-user escape hatch.

| Scenario | Connection | TLS | Auth |
|----------|-----------|-----|------|
| Local | `127.0.0.1` | None | None |
| Remote self-hosted | Relay via loopflow.studio | loopflow.studio terminates | JWT |
| Remote loopflow-hosted | Relay via loopflow.studio | loopflow.studio terminates | JWT |

This resolves:
- **Connection routing**: Relay handles NAT (lfd initiates outbound tunnel)
- **Multiple networks**: Not relevant for relay model
- **Relay fallback**: Relay is the default, not fallback

**Token validation**: Local JWT validation using cached JWKS. Connection tokens are short-lived JWTs (5 min). No roundtrip to loopflow.studio per request.

**Self-signed certs**: Only for direct connections (power users). Relay connections use loopflow.studio's real TLS certs.

## lfd Registration (Implementation assumptions)

- **JWT storage contract**: Assumed `~/.lf/credentials.json` contains a `jwt` or `token` string. Confirm actual key name and file format.
- **gRPC auth metadata**: Assumed connection token arrives as `authorization: Bearer <token>` or `x-loopflow-connection-token`. Confirm the agreed header name.
- **Auth gating behavior**: Local clients (`127.0.0.1`) bypass auth always. Remote connections through relay require valid JWT.

## gRPC Terminal Streaming

1. **Buffer size tuning**: 100KB ring buffer is a guess for reconnection replay. Should we make it configurable? Profile memory impact with many concurrent sessions?

2. **Heartbeat/keepalive**: Should we add explicit heartbeat messages to TerminalStream, or rely on HTTP/2 PING frames? Mobile networks aggressively drop idle connections.

3. **Session timeout**: If mobile disconnects and doesn't reconnect within X minutes, should lfd kill the agent? Or let it run indefinitely (current behavior for ConnectWave)?

4. **Multiple clients same session**: Can two mobile clients connect to the same terminal session simultaneously? Current design says no (single subscriber). If we allow multiple, we'd need to coordinate input and prevent echo storms.
