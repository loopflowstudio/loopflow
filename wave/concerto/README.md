# Concerto

## Vision

The app humans use to steer agents. Mobile experience, voice input, interaction patterns, app-level polish. Not every screen in the app — vertical feature UI (chords viz, cross-repo portfolio, context breakdown) lives with its domain wave.

Concerto is one multiplatform app — Mac, iPad, iPhone. Mobile is a fast check-in surface, not a shrunken desktop.

### Not here

- Vertical feature UI (chord grouping, portfolio edges, context breakdown — those live with Scale and Context)
- Backend work that happens to have a UI
- Bundled daemon behavior (that's Foundation)

## Strategy

Start with small, low-risk items that teach the Swift codebase (queue management, API key entry), then build up to more complex interaction patterns (release UI, voice auto-send).

## Goals

- Mobile reply workflows are fluid — reorder, edit, delete queued replies
- API key entry works without CLI
- Release config and "Release Now" accessible from the app
- Voice conversation loop works hands-free

## Risks

- iOS and macOS view divergence increases per-feature surface area
- SwiftUI multiplatform behavior still differs in navigation and selection APIs
- Voice accuracy on technical terms (lfd, worktree, etc.)
- VAD false positives in noisy environments

## Metrics

- % of LoopflowCore code shared between macOS and iOS (target: >80%)
- Time from tap-to-talk to message sent (target: <3s)
- VAD false activation rate (target: <2/hr)
- Correction rate for voice transcriptions (target: <20%)
