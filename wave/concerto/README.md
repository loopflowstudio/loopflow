# Concerto

## Vision

The app humans use to steer agents.

Mobile experience, voice input, interaction patterns, and app-level polish live here. Vertical feature UI (chords viz, cross-repo portfolio, context breakdown) stays with its domain wave.

Concerto is one multiplatform app — Mac, iPad, iPhone. Mobile is a fast check-in surface, not a shrunken desktop.

### Not here

- Vertical feature UI (chord grouping, portfolio edges, context breakdown — those live with Scale and Context)
- Backend work that happens to have a UI
- Bundled daemon behavior (that's Foundation)

## Strategy

Finish the remaining user-facing gaps in the order that teaches the most with the least churn.

Secrets provider shipped Doppler integration across lfd, HTTP API, and both Concerto shells. The connections panel redesign shipped alongside it — providers are now grouped by role (Agents, Source Control, Project Management, Secrets) with `ProviderRow` replacing `AuthProviderCard` and `ConnectionsPanel` available in both repo settings and a new Portfolio sheet. Per-repo enable/disable infrastructure is wired but the persistence toggle is deferred. CLI commands (`lf auth doppler`) and periodic auto-sync are deferred.

Release UI comes next because the underlying release commands already exist; the missing work is a clear operator surface, not new release mechanics. Auto-send comes last because the voice stack already records, transcribes, and resumes listening — the hard part left is making silence-based sending trustworthy without surprising people.

Keep shared SwiftUI and LoopflowCore surfaces dominant. If a feature forces large macOS/iOS forks, the design is probably wrong.

## Goals

- API-key-backed providers connect through secrets sync instead of manual key paste
- Release workflows are visible and runnable from Concerto
- Voice input can progress from speech to send to resumed listening without a keyboard
- Shared multiplatform code stays the default path for new Concerto features

## Risks

- Doppler device-flow auth and project/config selection may not fit the current broker model cleanly
- SwiftUI interaction and accessibility behavior still diverge across iPhone, iPad, and macOS
- Auto-send can destroy trust quickly if silence detection or cancel affordances are wrong
- Release UI could couple itself too tightly to unstable CLI contracts instead of thin backend primitives

## Metrics

- Shared LoopflowCore coverage for Concerto-facing features: >80%
- Manual API key pastes in supported setup flows: 0
- Release workflow launch from repo detail: <=2 interactions
- Time from speech stop to message send in continuous mode: <3s
- Voice correction rate during dogfood sessions: <20%
