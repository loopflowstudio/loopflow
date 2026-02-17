# Phase 05 Review — Concerto Remote Connection

## What was implemented

Concerto can now connect to a remote lfd daemon over HTTPS/WSS with explicit auth and TLS pinning. Previously Concerto only worked against localhost.

Concrete additions:

- **Connection model** (`ServerConnection`, `ConnectionState`, `RepoTarget`, `RemoteRepo`) — first-class types for host/port/TLS/auth, with URL generation and connection-key derivation.
- **Connection-driven services** — `WaveService` and `EventService` accept a `ServerConnection` and derive URL scheme, auth headers, and timeout tiers from it. No separate local/remote code paths.
- **TLS TOFU certificate pinning** — `CertificatePinningDelegate` pins on first connect, fails closed on fingerprint mismatch. `CertificatePinStore` persists fingerprints in UserDefaults. Trust/reset actions surface in UI.
- **Deterministic handshake** — `tlsTrustCheck → authCheck → repoDiscovery → wsProbe`. Each phase maps errors to a distinguishable `ConnectionState`.
- **WAN-friendly reconnect** — Exponential backoff with jitter (capped at 30s), immediate retry on NWPathMonitor network restore.
- **Remote repo selection** — `GET /v0/repos` endpoint on lfd aggregates repos by path. Concerto discovers available repos after connecting.
- **Connection settings UI** — `ConnectionSettingsView` with host/port/TLS/auth form, connect/test button, trust/reset actions, switch-to-local.
- **Local-action gating** — Terminal panel, filesystem typeahead, and reveal-in-finder are disabled when connected remotely.
- **Deploy stack** — Caddyfile with internal TLS + docker-compose prod overlay setting `LFD_AUTH_PROVIDER: static`.
- **Security wave plan** — Six-phase hardening roadmap covering loopback auth, path validation, container hardening, API surface gating, credential hygiene, and auth provider isolation.

## Key choices

| Decision | Why | Alternative considered |
|----------|-----|----------------------|
| Single `ServerConnection` struct controls everything | Auth/TLS/timeout derived from explicit fields, not inferred from hostname. Eliminates implicit coupling. | Separate `RemoteConfig` + `LocalConfig` types — rejected as unnecessary complexity for one connection. |
| TOFU pinning, not CA validation | Self-signed certs are the common case for `tls internal`. CA validation would reject them. TOFU is practical and matches SSH behavior. | Full CA chain validation — deferred, not needed for Phase 05 deployments. |
| Connection state as explicit enum | Distinguishes auth failure, trust mismatch, network error, reconnecting. UI can show precise status. | Boolean `isConnected` — too coarse, hides actionable differences. |
| `WaveService`/`EventService` are connection-driven, not subclassed | Same code path for local and remote. Connection fields control behavior. | Protocol + LocalImpl + RemoteImpl — rejected as premature; the differences are just URL scheme and timeout tiers. |
| Security wave as separate `wave/security/` directory | Security concerns cross-cut remote/infra/app. Own directory keeps them discoverable. | Inline in `wave/remote/` — rejected because security items are independently scoped and prioritized. |

## How it fits together

```
ConnectionStore (persistence, keychain, pin store)
        ↓ activeConnection
    RepoState (orchestration: handshake, events, wave CRUD)
        ↓ builds
  WaveService ──→ lfd HTTP (GET/POST/PATCH/DELETE)
  EventService ──→ lfd WebSocket (subscribe, reconnect)
        ↓ feeds
  ConnectionSettingsView (UI: form, connect, trust actions)
  WaveSidebar (connection indicator, create gating)
  WaveDetailPanel / AreaTypeahead (remote-action gating)
```

`ConnectionStore` owns persistence (UserDefaults for connection config, Keychain for tokens, UserDefaults for certificate pins). `RepoState` owns the handshake and reconnect orchestration, delegates to services. Services are stateless structs that take a connection and token provider.

## Risks and bottlenecks

- **RepoState is large** (~880 lines). It owns both connection orchestration and wave CRUD. A follow-up could extract connection lifecycle into `ConnectionStore` itself, but this is a structure concern, not a correctness risk.
- **TOFU pinning trusts first certificate unconditionally**. Acceptable for `tls internal` and pre-shared-key deployments. Public CA use cases would benefit from optional CA validation (not in Phase 05 scope).
- **Token provider is a closure snapshot**. `WaveService` captures the token at construction time via closure. If the token is rotated, services must be rebuilt. This is handled by `rebuildServices()` on connection change, but a leaked old service reference would use a stale token.
- **`pinningDelegate` computed property creates a new delegate per request** in `WaveService`. This is intentional (each URLSession needs its own delegate), but means fingerprint mismatch detection is per-request. A mismatch on request N doesn't prevent request N+1 from also failing — the trust state surfaces via `ConnectionState`.

## What's not included

- **Multi-server management** — One active connection at a time.
- **Studio/JWT auth** — Only `none` and `staticToken` modes. JWT lifecycle is Phase 07.
- **Remote file browsing/typeahead** — AreaTypeahead falls back to a plain text field for remote. Full file browsing is Phase 08.
- **Remote terminal/editor launch** — Disabled for remote targets. SSH-based launch is Phase 06.
- **Repo-less startup flow** — Concerto still enters remote mode from an existing repo window.
