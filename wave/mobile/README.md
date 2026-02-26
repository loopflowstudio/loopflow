# Mobile

Concerto on iPhone and iPad. Action-button-first interaction. Multi-client session continuity.

## Vision

Concerto becomes a single multiplatform app — Mac, iPad, iPhone. The mobile experience is not a shrunken desktop; it's a choose-your-own-adventure interface where agents surface next actions as tappable buttons. Chat is available but secondary. You start a wave on your Mac, check on it from your phone, tap "Land PR" from the couch.

LoopflowCore holds shared state, models, services, and reusable views. Both Concerto and Symphonia (separate repo) depend on it.

### Not here

- Embedded terminal on iOS (Ghostty is macOS-only; mobile gets output view, not a shell)
- Step runner / typeaheads on phone (tablet maybe, phone no)
- Offline mode (lfd connection required)
- App Store distribution (TestFlight first)

## Goals

- One Concerto target, three form factors (Mac unchanged, iPad close to Mac, iPhone minimal)
- LoopflowCore as shared library: models, services, and reusable views
- Action buttons as the primary mobile interaction — agents suggest next actions, users tap
- Multi-client: both devices connect to same lfd, session state is server-side
- Zero-config discovery: lfd publishes to studio, mobile auto-discovers on login

## Multiplatform guardrails

- Keep `#if` footprint low: platform checks belong in app entry wiring and platform shell files only.
- Keep shared state/views/models macro-free in `LoopflowCore`.
- Put platform behavior behind injected capabilities, not inline platform checks.
- Use file-structure boundaries: `Concerto/Platform/macOS` and `Concerto/Platform/iOS`.
- iOS is remote-client only — no bundled local daemon on iPhone/iPad.
- Boundary enforcement: `check_swift_multiplatform_boundaries.py` blocks macOS-only imports in LoopflowCore.

## Risks

- iOS and macOS views diverging means feature work touches both platforms — more surface area per feature. Purpose-built views are simpler than forced sharing.
- Action button quality depends on agent prompt engineering — bad suggestions = bad UX.
- SwiftUI multiplatform has rough edges (NavigationSplitView behaves differently on iOS)
- Tailscale as prerequisite narrows the audience — users must install it on both devices
