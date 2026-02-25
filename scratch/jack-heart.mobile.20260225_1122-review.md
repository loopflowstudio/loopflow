# Review: mobile action buttons (branch `jack-heart.mobile.20260225_1122`)

## What was implemented

- Added engine-level synthetic tool injection (`SyntheticTool`, `ClientContext`, `synthetic_tools_for_context`) and plumbed it through launch/session setup.
- Added `suggest_actions` guidance injection for UI-backed sessions, with compact/regular action count limits and step `action_style` guidance.
- Added `action_style` parsing on steps and propagated it through prompt preparation.
- Added `<lf:suggest_actions>` tag parsing in session harness streaming (`LfTagParser`) and emitted typed `SessionEvent::SuggestedActions` events.
- Extended session config/event models in Rust + Swift for `client_has_ui`, `client_compact`, and suggested action payloads.
- Added shared Swift suggested action state/model + sanitization/clearing rules, and new `ActionButtonsView` rendered above composer in `WaveChatView`.
- Added tap-to-send behavior for suggested actions (`sendSuggestedAction`) and state clearing on send/typing/new turns/session end.
- Added/updated tests across Rust and Swift for synthetic tool selection, tag parsing, event mapping, and chat-state behavior.
- Polish fix in this gate pass: latest suggested-actions payload now correctly replaces prior actions even when payload sanitizes to empty.

## Key choices

- **Engine-owned injection** over client-only prompt hacks so synthetic tools can scale beyond `suggest_actions`.
- **Tag-based tool realization** (`<lf:...>`) instead of MCP registration for provider portability and lower complexity.
- **Typed event propagation** (`SuggestedActions`) instead of client-side text scanning.
- **Aggressive action clearing** to avoid stale/incorrect button suggestions.
- **Latest payload wins** semantics in ChatState, including empty payload replacement.

## How it fits together

Session creation now carries client UI context (`client_has_ui`, `client_compact`) into launch prep. The engine injects `suggest_actions` synthetic guidance into system prompts when UI is present. Providers stream text; harness parsers strip `<lf:suggest_actions>` payloads and emit structured `SuggestedActions` events. Swift `ChatState` sanitizes/caps/replaces action state, and `WaveChatView` renders tappable buttons that route through the same send path as typed input.

## Risks and bottlenecks

- Prompt-instruction compliance still depends on model behavior; no hard provider-side tool contract.
- Tag parsing is robust for streaming splits but still a text-protocol surface (malformed payloads are ignored).
- iOS/macOS UX quality depends on suggestion relevance; stale/noisy suggestions would reduce trust.
- Xcode UI test target currently fails locally during link (`ConcertoUITests`, `open() failed, errno=1`), so full UI-suite validation remains blocked in this environment.

## What's not included

- No Codex-specific synthetic tool realization beyond shared prompt guidance path.
- No additional synthetic tools (e.g. `memory`) yet.
- No persistence/analytics/ranking system for suggested actions.
- No transport/protocol redesign beyond the added typed event payloads.

## Validation run

- `cargo fmt --all -- --check` ✅
- `cargo clippy --all-targets -- -D warnings` ✅
- `cargo test --all` ⚠️ fails locally only on 2 Docker-socket-dependent tests (no `/var/run/docker.sock`)
- `cargo test --all -- --skip lfd::executor::docker::tests::docker_startup_lost_agent_does_not_flip_terminal_run_wave_status --skip lfd::executor::docker::tests::docker_startup_rehydrates_running_agents_and_cleans_orphans` ✅
- `uv run pytest python/tests/` ✅
- `swift test --package-path swift` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ⚠️ fails locally at `ConcertoUITests` link step (`open() failed, errno=1`)
