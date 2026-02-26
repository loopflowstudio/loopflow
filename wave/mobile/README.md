# Mobile

Concerto on iPhone and iPad. Action-button-first interaction, shared session state, and direct connection to lfd.

## Vision

Concerto is one multiplatform app — Mac, iPad, iPhone. Mobile is not a shrunken desktop: it is a fast check-in surface where agents suggest next actions and users can act in a tap or two.

LoopflowCore holds shared state, models, and services. Platform shells stay thin and purpose-built (`Concerto/Platform/macOS`, `Concerto/Platform/iOS`).

## Core components

- **LoopflowCore**: shared session/wave models, networking, and reusable UI primitives.
- **iOS shell**: touch-first views for discovery, wave detail, output, and session interaction.
- **lfd + studio discovery**: lfd publishes presence metadata (`url`, `repos`) to studio; mobile discovers and connects directly.

## Invariants

- Discovery is additive. Manual host/port connection remains available.
- iOS is remote-client only; no bundled daemon on phone/tablet.
- Shared core stays platform-agnostic; platform checks live in platform shells and app wiring.
- lfd remains the source of truth for session and wave state; clients render and send intent.

## Differentiators

- Action buttons are the primary interaction path on mobile.
- Discovery reduces setup friction without introducing a studio relay path.
- Multi-client continuity: start on Mac, continue from iPhone/iPad against the same server-side state.

## Goals

- Keep Mac behavior stable while iOS/iPad UX evolves independently.
- Keep shared models/services in LoopflowCore and avoid cross-platform drift in protocol behavior.
- Make session feedback workflows (quote replies) work reliably on touch devices.

## Risks

- iOS and macOS view divergence increases per-feature surface area.
- Action quality depends on model prompt adherence.
- SwiftUI multiplatform behavior still differs in navigation and selection APIs.
- Tailscale remains a prerequisite for remote discovery-based connectivity.
- Discovery introduces an additional lfd→studio validation hop during connect.
