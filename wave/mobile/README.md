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
- Zero-config discovery: lfd publishes to studio, mobile auto-discovers on login, connects via Tailscale

## Phase boundaries

- **01-multiplatform**: ~~shared core extraction +~~ iOS shells + manual remote connection + design token extraction + boundary enforcement + macOS file migration to Platform/macOS/. No discovery. *Shipped. State extracted to LoopflowCore. iOS got purpose-built views. macOS files migrated. Mixed-platform files (LiveOutput, WaveSessionView) left in place intentionally — they have partial guards, not whole-file gates.*
- **02-action-buttons**: `suggest_actions` UX backed by shared ActionButtonsView component. No discovery and no multi-client protocol work. *Shipped. Full engine pipeline (StructuredReply, ClientContext, LfTagParser, SessionEvent) + shared Swift model + ActionButtonsView in LoopflowCore + macOS WaveSessionView integration. Chat→Session rename across Swift (ChatState→SessionState, WaveChatView→WaveSessionView).*
- **03-multi-client**: reliability and correctness for concurrent clients on one lfd. Manual connection path remains primary. *Complete. iOS suggested-action rail, concurrent-client backend coverage (`tests/e2e/test_concurrent_clients.py`), iOS foreground reconnect (`checkConnectionHealth` + `resumeFromBackground`), cross-client stale-action clearing on `turnStarted`, and visual verification script (`scripts/verify_mobile_stage03.py`).*
- **04-lfd-discovery**: optional zero-config discovery via studio + Tailscale, additive to manual host/port connection.
- **05-quote-replies**: inline quote-reply UX for long session responses (highlight, react/reply, queue, send as one structured message). *macOS-first milestone shipped (demo panel + live WaveSessionView wiring + assembly tests). iOS selection gesture support, queue reorder/edit, and rich markdown selectable rendering remain follow-up work.*

## Multiplatform guardrails

- Keep `#if` footprint low: platform checks belong in app entry wiring and platform shell files only.
- Keep shared state/views/models macro-free in `LoopflowCore`.
- Put platform behavior behind injected capabilities, not inline platform checks.
- Use file-structure boundaries: `Concerto/Platform/macOS` and `Concerto/Platform/iOS`.
- iOS is remote-client only — no bundled local daemon on iPhone/iPad.
- Boundary enforcement: `check_swift_multiplatform_boundaries.py` blocks macOS-only imports in LoopflowCore.

## Risks

- ~~Platform-gating macOS code (Ghostty, Carbon, NSOpenPanel, keyboard router) may be messier than expected~~ *Resolved: manageable with `#if os(macOS)` at file level and platform-conditional Package.swift settings.*
- ~~Moving views into LoopflowCore requires careful dependency management (views can't depend on app-level state)~~ *Resolved differently: iOS got purpose-built views instead. Shared components (ActionButtonsView, DesignSystem) live in LoopflowCore; platform views stay in Concerto.*
- iOS and macOS views diverging means feature work (like action buttons) touches both platforms — more surface area per feature. *Confirmed by Stage 01: this is real but manageable. Purpose-built views are simpler than forced sharing.*
- ~~ConnectionProfile abstraction needed early for iOS connection management~~ *Dropped: tried in Stage 01, removed. Simple ConnectionMode (bundled/remote) + ConnectionStore suffices. Profiles may return in Stage 03 if saved connections become important.*
- Action button quality depends on agent prompt engineering — bad suggestions = bad UX. *Confirmed by Stage 02: prompt-compliance-dependent, no strict tool contract. Manageable but real — quality is only as good as the model's adherence to guidance. Pipeline is proven across both Claude and Codex harnesses, so provider portability is not a concern.*
- Harness layer has subtle assumptions about message authorship — Stage 02 found an auto-send bug where claude_mapping.rs echoed user text as new messages. Fixed, but multi-client work (Stage 03) should audit message attribution paths carefully.
- ~~Suggested actions are client-side ephemeral state~~ *Resolved: `SessionState.reduce()` clears suggested actions on `.turnStarted` — stale suggestions clear on all connected clients when any client starts a turn.*
- ~~Multi-client session handoff may need lfd changes if sessions assume single-client~~ *Mitigated: concurrent-client fanout/replay behavior is now covered by `tests/e2e/test_concurrent_clients.py` and runs in CI.*
- SwiftUI multiplatform has rough edges (NavigationSplitView behaves differently on iOS)
- Tailscale as prerequisite narrows the audience — users must install it on both devices
- Studio discovery service is simple but is still new infrastructure to operate

## Metrics

- ~~Concerto builds and runs on iOS Simulator~~ *Done (iPhone 17, iPad Pro 11-inch M5).*
- Can see wave list, tap into a wave, see live output on iPhone *(builds confirmed; interactive validation against live lfd pending — blocked on headless simulator interaction primitives)*
- ~~Agent surfaces action buttons, tapping one sends the message~~ *Done on both macOS and iOS. iOS now exposes a persistent bottom action rail in MobileWaveDetailView and uses the same `SessionState.sendSuggestedAction` send path as macOS. Remaining check: manual on-screen simulator validation for iPhone/iPad thumb-zone spacing and tap ergonomics.*
- ~~Session feedback can be attached to exact assistant spans via quote-replies~~ *Partially done. macOS supports selectable assistant quotes, emoji/text replies, queue tray, and structured send assembly. iOS quote selection and queued-entry editing/reordering are not shipped yet.*
- ~~Same lfd instance serves both Mac and iOS Concerto simultaneously~~ *Done. Backend fanout covered by `tests/e2e/test_concurrent_clients.py`. iOS foreground reconnect ships stale state recovery. `scripts/verify_mobile_stage03.py` covers manual device validation.*
- Login on mobile → see running lfds → tap to connect (no manual IP/port entry)
