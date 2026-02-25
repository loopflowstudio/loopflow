# Mobile stage status and next work (2026-02-25)

This branch completed Stage 01 multiplatform cleanup and is ready for Stage 02 action buttons.

## Current state

### Stage 01 completed

- iOS shell flow shipped in Concerto (`ConnectionSetupView` → wave list → wave detail).
- Shared state/services now live in `LoopflowCore` and are used by both iOS and macOS.
- macOS-only files were migrated into `Concerto/Platform/macOS/**` when they were whole-file `#if os(macOS)` wrappers.
- Mixed-platform files with partial guards were intentionally left in place:
  - `Concerto/Views/LiveOutput.swift`
  - `Concerto/Views/WaveChatView.swift`
- `swift/project.yml` now keeps `Platform/macOS` sources macOS-only via destination filters.
- Boundary guardrail added: `scripts/check_swift_multiplatform_boundaries.py` (also wired into CI).
- Dev script renamed and consolidated to `scripts/concerto-dev.py`.

### Verification completed

- `swift test --package-path swift` ✅
- `uv run python scripts/check_swift_multiplatform_boundaries.py` ✅
- `scripts/concerto-dev.py run-ios` builds and launches in Simulator ✅

## Remaining Stage 01 gap

Interactive iOS tap-through against live `lfd` is still not complete in this headless environment.

Validated so far:
- local `lfd` served on `127.0.0.1:2486`
- test wave seeded (`mobile-e2e-test`)
- app launch/build path confirmed

Blocked:
- no reliable simulator UI interaction primitive here for entering fields and tapping controls

## Stage 02 active target: action buttons

Primary goal: make mobile follow-up turns tap-first via `suggest_actions`.

### Implement

1. Add `SuggestedAction` in `LoopflowCore` and `ChatState.suggestedActions`.
2. Parse `tool_use` items where `name == "suggest_actions"`.
3. Enforce lifecycle rules:
   - clear on user send
   - clear on composer typing start
   - clear on agent turn start
   - clear on session/connection reset
4. Add shared `ActionButtonsView` in `LoopflowCore`.
5. Embed in both chat surfaces:
   - iOS `MobileWaveDetailView`
   - macOS `WaveChatView`
6. Tap behavior: send button `label` exactly as user message; clear optimistically on tap.

### Constraints

- No new wire format.
- Stage 02A first: prompt/tool convention; 02B tool registration in `lfd` after 02A works.
- Max 4 rendered actions; malformed entries ignored.

### Done when

- Buttons render on iPhone, iPad, and Mac from `suggest_actions` tool calls.
- Tapping sends the label as the next user message.
- Buttons clear on tap/typing/new agent turn.
- Regression checks pass:
  - `swift test --package-path swift`
  - `uv run python scripts/check_swift_multiplatform_boundaries.py`

## Canonical long-form specs

- `wave/mobile/01-multiplatform.md`
- `wave/mobile/02-action-buttons.md`
- `wave/mobile/03-multi-client.md`
- `wave/mobile/04-lfd-discovery.md`
