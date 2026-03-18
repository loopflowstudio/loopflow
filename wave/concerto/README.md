# Concerto

## Vision

The app humans use to steer agents. Mobile experience, voice input, interaction patterns, app-level polish, and getting the app onto real devices. Not every screen in the app — vertical feature UI (chords viz, cross-repo portfolio, context breakdown) lives with its domain wave.

Concerto is one multiplatform app — Mac, iPad, iPhone. Mobile is a fast check-in surface, not a shrunken desktop.

### Not here

- Vertical feature UI (chord grouping, portfolio edges, context breakdown — those live with Scale and Context)
- Backend work that happens to have a UI
- Bundled daemon behavior (that's Foundation)

## Strategy

Two items, increasing scope. Secrets provider introduces cross-cutting infrastructure (Rust trait, OAuth flow, sync, CLI, UI). Release UI builds on existing ops commands with a focused Concerto surface.

## Roadmap

1. **Secrets provider** — Abstract secrets provider trait, Doppler as first implementation. OAuth into Doppler, lfd syncs API keys to harness providers automatically.
2. **Release UI** — Per-repo release config and "Release Now" button with version picker.

## Goals

- API key management works through secrets providers, not manual paste
- Release config and "Release Now" accessible from the app

## Risks

- Doppler OAuth device flow may have quirks not covered by our existing auth pattern
- iOS and macOS view divergence increases per-feature surface area
- SwiftUI multiplatform behavior still differs in navigation and selection APIs

## Metrics

- % of LoopflowCore code shared between macOS and iOS (target: >80%)
