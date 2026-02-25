# Mobile

Concerto on iPhone and iPad. Action-button-first interaction. Multi-client session continuity.

## Vision

Concerto becomes a single multiplatform app — Mac, iPad, iPhone. The mobile experience is not a shrunken desktop; it's a choose-your-own-adventure interface where agents surface next actions as tappable buttons. Chat is available but secondary. You start a wave on your Mac, check on it from your phone, tap "Land PR" from the couch.

LoopflowCore expands to hold shared views alongside models and services. Both Concerto and Symphonia (separate repo) depend on it — shared component library.

Role models: ChatGPT and Claude iOS apps for navigation patterns (tab bar, conversation list → detail, suggested replies).

### Not here

- Embedded terminal on iOS (Ghostty is macOS-only; mobile gets output view, not a shell)
- Step runner / typeaheads on phone (tablet maybe, phone no)
- Offline mode (lfd connection required)
- App Store distribution (TestFlight first)

## Goals

- One Concerto target, three form factors (Mac unchanged, iPad close to Mac, iPhone minimal)
- LoopflowCore as shared library: models, services, and reusable views — consumed by Concerto and Symphonia
- Action buttons as the primary mobile interaction — agents suggest next actions, users tap
- `suggest_actions` tool convention on top of existing session protocol — no new wire format
- Multi-client: both devices connect to same lfd, session state is server-side
- Zero-config discovery: lfd publishes to studio, mobile auto-discovers on login, connects via Tailscale

## Phase boundaries

- **01-multiplatform**: shared core extraction + iOS shells + manual remote connection. No discovery.
- **02-action-buttons**: `suggest_actions` UX in shared chat surfaces. No discovery and no multi-client protocol work.
- **03-multi-client**: reliability and correctness for concurrent clients on one lfd. Manual connection path remains primary.
- **04-lfd-discovery**: optional zero-config discovery via studio + Tailscale, additive to manual host/port connection.

## Multiplatform guardrails

- Keep long-term `#if` footprint low: platform checks belong in app entry wiring and platform shell files only.
- Keep shared state/views/models macro-free in `LoopflowCore`.
- Put platform behavior behind injected capabilities (daemon, notifications, external actions), not inline platform checks.
- Use file-structure boundaries: `Concerto/Platform/macOS` and `Concerto/Platform/iOS`.
- iOS Stage 01 is remote-client only. No bundled local daemon on iPhone/iPad.
- Add boundary checks in CI/scripts: block macOS-only imports in `LoopflowCore` and net-new non-shell `#if`.

## Risks

- Platform-gating macOS code (Ghostty, Carbon, NSOpenPanel, keyboard router) may be messier than expected
- Moving views into LoopflowCore requires careful dependency management (views can't depend on app-level state)
- Action button quality depends on agent prompt engineering — bad suggestions = bad UX
- Multi-client session handoff may need lfd changes if sessions assume single-client
- SwiftUI multiplatform has rough edges (NavigationSplitView behaves differently on iOS)
- Tailscale as prerequisite narrows the audience — users must install it on both devices
- Studio discovery service is simple but is still new infrastructure to operate

## Metrics

- Concerto builds and runs on iOS Simulator
- Can see wave list, tap into a wave, see live output on iPhone
- Agent surfaces action buttons, tapping one sends the message
- Same lfd instance serves both Mac and iOS Concerto simultaneously
- Login on mobile → see running lfds → tap to connect (no manual IP/port entry)
