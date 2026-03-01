# Mobile

Concerto on iPhone and iPad. Action-button-first interaction, shared session state, and direct connection to lfd.

## Vision

Concerto is one multiplatform app — Mac, iPad, iPhone. Mobile is not a shrunken desktop: it is a fast check-in surface where agents suggest next actions and users can act in a tap or two.

### Not here

- Bundled daemon on phone/tablet (iOS is remote-client only)
- Studio relay path for discovery (discovery is additive to manual host/port)
- TLS certificate loading and serving (separate work; `--allow-insecure-bind` is the current escape hatch for `0.0.0.0` bind)
- Per-repo or per-capability token scoping
- Auto-revoke on suspicious patterns (same token from multiple IPs)
- `advertise_url` config for reverse proxy setups
- Tailscale LocalAPI migration

## Strategy

LoopflowCore holds shared state, models, and services. Platform shells stay thin and purpose-built (`Concerto/Platform/macOS`, `Concerto/Platform/iOS`). lfd remains the source of truth — clients render and send intent. Discovery via lfd presence metadata reduces setup friction without introducing relay dependencies.

Action buttons are the primary interaction path on mobile. Multi-client continuity: start on Mac, continue from iPhone/iPad against the same server-side state.

## Goals

- Keep Mac behavior stable while iOS/iPad UX evolves independently
- Keep shared models/services in LoopflowCore and avoid cross-platform drift in protocol behavior
- Make session feedback workflows (quote replies) work reliably on touch devices

## Risks

- iOS and macOS view divergence increases per-feature surface area
- Action quality depends on model prompt adherence
- SwiftUI multiplatform behavior still differs in navigation and selection APIs
- Tailscale remains a prerequisite for remote discovery-based connectivity
- Studio dependency for token distribution: lfd validates locally, but studio needs pool storage and token handout endpoints (don't exist yet) for end-to-end flow
- WS re-validation (60s interval in WebSocket select loop) is a new pattern on a critical path — if `validate()` is slow, it could stall the event stream
- SQLite sidecar for token ledger (`~/.lf/connection_tokens.db`) is a second data store alongside Postgres; acceptable for v1 since tokens are ephemeral (1-hour TTL)

## Metrics

- % of LoopflowCore code shared between macOS and iOS vs platform-specific (target: >80% shared)
- Time from app launch to wave list visible on mobile: seconds (target: <3s)
- Action button tap-to-response latency: seconds from tap to visible agent response (target: <2s)
- Number of platform-specific SwiftUI workarounds required per release (track to gauge multiplatform friction)
