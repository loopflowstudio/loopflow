# Mobile

## Vision

Concerto on iPhone and iPad. Action-button-first interaction. Multi-client session continuity. Not an embedded terminal on iOS, not a step runner on phone, not offline mode, not App Store distribution yet.

Concerto becomes a single multiplatform app — Mac, iPad, iPhone. The mobile experience is not a shrunken desktop; it's a choose-your-own-adventure interface where agents surface next actions as tappable buttons. Chat is available but secondary. You start a wave on your Mac, check on it from your phone, tap "Land PR" from the couch.

## Strategy

LoopflowCore holds shared state, models, services, and reusable views. Both Concerto and Symphonia (separate repo) depend on it. iOS is remote-client only — no bundled local daemon on iPhone/iPad.

Multiplatform (Phase 01), action buttons (Phase 02), and multi-client (Phase 03) shipped — state extracted to LoopflowCore, iOS got purpose-built views, action button pipeline (StructuredReply → ClientContext → LfTagParser → SessionEvent → ActionButtonsView), iOS suggested-action rail, concurrent-client backend coverage, iOS foreground reconnect, cross-client stale-action clearing.

Quote-replies (Phase 05) shipped macOS-first — demo panel, live WaveSessionView wiring, assembly tests. iOS selection gesture support, queue reorder/edit, and rich markdown selectable rendering remain follow-up.

### Multiplatform guardrails

- Keep `#if` footprint low: platform checks belong in app entry wiring and platform shell files only.
- Keep shared state/views/models macro-free in `LoopflowCore`.
- Put platform behavior behind injected capabilities, not inline platform checks.
- Use file-structure boundaries: `Concerto/Platform/macOS` and `Concerto/Platform/iOS`.
- Boundary enforcement: `check_swift_multiplatform_boundaries.py` blocks macOS-only imports in LoopflowCore.

## Goals

- One Concerto target, three form factors (Mac unchanged, iPad close to Mac, iPhone minimal)
- LoopflowCore as shared library: models, services, and reusable views
- Action buttons as the primary mobile interaction — agents suggest next actions, users tap
- Multi-client: both devices connect to same lfd, session state is server-side
- Zero-config discovery: lfd publishes to studio, mobile auto-discovers on login, connects via Tailscale

## Risks

- iOS and macOS views diverging means feature work touches both platforms — more surface area per feature. Confirmed: real but manageable. Purpose-built views are simpler than forced sharing.
- Action button quality depends on agent prompt engineering — bad suggestions = bad UX. Quality is only as good as the model's adherence to guidance.
- SwiftUI multiplatform has rough edges (NavigationSplitView behaves differently on iOS)
- Tailscale as prerequisite narrows the audience — users must install it on both devices
- Studio discovery service is simple but is still new infrastructure to operate

## Metrics

- Can see wave list, tap into a wave, see live output on iPhone
- Agent surfaces action buttons, tapping one sends the message — on both macOS and iOS
- Same lfd instance serves both Mac and iOS Concerto simultaneously
- Login on mobile → see running lfds → tap to connect (no manual IP/port entry)
