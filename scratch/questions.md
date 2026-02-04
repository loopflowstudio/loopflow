# Open Questions

Questions that emerged during implementation. Need resolution before or during next iteration.

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

## Auth Implementation

- Refresh endpoint response: assumed /auth/refresh returns JSON {token|jwt} or raw token string; confirm actual contract.
- Package supports macOS 15 only; design doc targets macOS 14+/iOS 17+. If iOS support is required, update Swift package platforms and add iOS app target.
- RemoteWaveService not present in repo; TokenProvider added but not wired into any remote service yet.

## lf ops (Rust)

1. **Rebase assistant prompt**: Rust ops uses a custom inline prompt instead of a dedicated `rebase` step file. There is no Rust builtin `rebase` step today. Should we add builtin ops steps to `loopflow-engine` and switch to them?
2. **Next branch naming**: `lf ops next` still uses `next-<timestamp>` instead of wave-based branch naming (no wave metadata update in Rust yet). Should we wire it to wave naming once the wave module is ported?
3. **Land local strategy**: `lf ops land --local` uses squash merge via `git::land` and skips PR-related steps. Confirm if we need a local merge option or to support `--strategy` explicitly.
4. **Lint fixer**: lint retry uses the built-in `lint` step via prompt context, not `lf lint -b` subprocess. Is that preferred for parity?

## lfd Registration — RESOLVED

**Decision**: Relay through loopflow.studio is the primary remote path. Direct connections are a power-user escape hatch.

| Scenario | Connection | TLS | Auth |
|----------|-----------|-----|------|
| Local | `127.0.0.1` | None | None |
| Remote self-hosted | Relay via loopflow.studio | loopflow.studio terminates | JWT |
| Remote loopflow-hosted | Relay via loopflow.studio | loopflow.studio terminates | JWT |

**Token validation**: Local JWT validation using cached JWKS. Connection tokens are short-lived JWTs (5 min). No roundtrip to loopflow.studio per request.

## lfd Registration (Implementation assumptions)

- **JWT storage contract**: Assumed `~/.lf/credentials.json` contains a `jwt` or `token` string. Confirm actual key name and file format.
- **gRPC auth metadata**: Assumed connection token arrives as `authorization: Bearer <token>` or `x-loopflow-connection-token`. Confirm the agreed header name.
- **Auth gating behavior**: Local clients (`127.0.0.1`) bypass auth always. Remote connections through relay require valid JWT.

## Phase 2: Non-interactive Mobile

New direction for mobile—no terminal streaming. See `scratch/concerto-mobile-direction.md`.

1. **iOS target setup**: Swift package needs iOS platform added. What's the minimum iOS version? (iOS 17+ for @Observable?)

2. **Remote lfd discovery**: Mobile needs to find user's lfd. Options:
   - User enters lfd URL manually
   - loopflow.studio provides discovery (requires registration)
   - For now: assume local network / same machine for testing

3. **HTTP vs gRPC for mobile**: Current lfd has HTTP API (port 2486) and gRPC (port 50051). Which should mobile use?
   - HTTP is simpler, works with URLSession
   - gRPC has better streaming for events
   - Recommendation: HTTP for actions, consider gRPC for event subscription later

4. **Push notification infrastructure**: Requires loopflow.studio server-side work. Should we defer push notifications and start with polling?

## Phase 3: Chat Experience (Future)

1. **API key management**: Where do users configure their Claude/OpenAI/Gemini API keys? Options:
   - In loopflow.studio account
   - In iOS app Keychain
   - Passed through lfd

2. **Context assembly**: How much context to include in chat? Need to balance relevance vs token cost.

3. **Conversation persistence**: Where to store chat history? Local on device? Synced via loopflow.studio?

## gRPC Terminal Streaming — DEFERRED

Terminal streaming is deferred in favor of non-interactive mobile + chat experience. See `roadmap/concerto/remote-terminal-view.md` for rationale.

If we revisit this later:
- Buffer size tuning (100KB guess)
- Heartbeat/keepalive for mobile networks
- Session timeout behavior
- Multiple clients same session
