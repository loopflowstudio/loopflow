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

## Risks

- Platform-gating macOS code (Ghostty, Carbon, NSOpenPanel, keyboard router) may be messier than expected
- Moving views into LoopflowCore requires careful dependency management (views can't depend on app-level state)
- Action button quality depends on agent prompt engineering — bad suggestions = bad UX
- Multi-client session handoff may need lfd changes if sessions assume single-client
- SwiftUI multiplatform has rough edges (NavigationSplitView behaves differently on iOS)

## Metrics

- Concerto builds and runs on iOS Simulator
- Can see wave list, tap into a wave, see live output on iPhone
- Agent surfaces action buttons, tapping one sends the message
- Same lfd instance serves both Mac and iOS Concerto simultaneously
