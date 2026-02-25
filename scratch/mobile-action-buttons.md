# 02: Action Buttons

## Problem

Chat sessions frequently end with the user thinking "what should I do next?" and then typing a command manually. That friction is worst on iPhone, where typing is slow and one-handed use is common.

We need an agent-driven, tappable next-step surface that:
- turns likely follow-up actions into one tap,
- works in the existing session protocol,
- behaves consistently on iPhone, iPad, and Mac.

Who benefits:
- mobile users running many short task loops,
- new users who do not know canonical next commands,
- desktop users who want faster repeat actions.

Why now:
- Stage 01 established shared chat state + platform embedding boundaries,
- this stage can ship value without waiting for discovery or transport work.

## Approach

Ship a **protocol-first action strip** in two phases, with the UI and state logic shared in LoopflowCore.

### Phase A (ship first): prompt-driven `suggest_actions`

1. Inject this instruction into `AgentSessionConfig.message` during session creation:
   - ask for 2–4 concrete next actions,
   - call `suggest_actions` after completing work or when waiting for user input,
   - prefer actionable labels ("Land PR") over vague labels ("Continue").
2. Do not wait for lfd tool registration; treat `suggest_actions` as a recognized tool name now.

### Phase B (follow immediately): formal tool registration

1. Add `suggest_actions` to lfd-provided tool definitions with schema validation.
2. Keep Phase A prompt guidance in place even after registration.

### Shared state and parsing (LoopflowCore)

1. Add `SuggestedAction` model (`label`, optional `description`, UUID id).
2. Add `suggestedActions: [SuggestedAction]` to `ChatState`.
3. Parse tool items where `name == "suggest_actions"` and input payload contains `actions`.
4. Payload handling rules:
   - keep only actions with non-empty `label`,
   - cap to 4 actions,
   - latest valid tool call replaces prior actions,
   - invalid payload yields no update.
5. Clear actions on:
   - user send,
   - agent turn start,
   - user begins manual typing,
   - session end/replay reset.

### UI behavior

1. Build shared `ActionButtonsView` in LoopflowCore.
2. Render above composer in chat surfaces (through `WaveChatView`; iOS entry point remains `MobileWaveDetailView`).
3. Layout:
   - compact width (iPhone): vertical full-width cards with optional subtitle,
   - regular width (iPad/Mac): horizontal wrapping pills.
4. Accessibility:
   - 44pt min touch target on iPhone,
   - visible focus ring on keyboard platforms,
   - VoiceOver reads label + description.
5. Interaction:
   - tap sends `label` through the exact same path as typed input,
   - actions disappear immediately after tap.

### Wild success target

Users begin treating the strip as the default continuation path: most follow-up turns after an assistant result are one tap, not manual typing.

### Wild failure to design against

Six months later we remove the feature because actions feel stale/noisy and users mistrust them. Mitigations in this design:
- aggressive clearing rules,
- strict payload sanitization,
- no fallback UI when agent does not suggest actions.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Text-only suggestions inside assistant messages | Zero protocol work | Not tappable; no structured parsing; weak mobile benefit |
| Client-generated suggestions (heuristics from transcript) | Works without agent tool use | Often wrong/out of context; hides model intent; harder to trust |
| Wait for full lfd/tooling changes before any UI | Cleaner architecture | Delays user value; blocks experimentation on copy/placement |

## Key decisions

- **Adopt `suggest_actions` immediately via system prompt (Phase A).** Speed to user value beats waiting for backend formalization.
- **Keep parsing in shared `ChatState`.** One behavior path for iOS + macOS avoids drift.
- **Latest call wins.** Action sets are ephemeral guidance, not history.
- **Clear aggressively.** Better to hide too early than show stale actions.
- **Mobile-first rendering.** Full-width vertical actions on compact width are the default experience.

## Scope

- In scope:
  - System prompt injection for `suggest_actions`
  - Shared `SuggestedAction` model + `ChatState` parsing/clearing
  - Shared `ActionButtonsView`
  - Embedding in iOS/macOS chat surfaces
  - Tap-to-send behavior parity with typed input
  - lfd schema registration as Phase B of this same stage

- Out of scope:
  - Action discovery/ranking systems (Stage 04)
  - New transport/wire formats (Stage 03)
  - Persistence/analytics dashboards for suggested actions
  - Multi-message or parameterized action payloads

## Done when

- Observable behavior:
  1. Agent emits a `suggest_actions` tool call and action buttons render in chat on iPhone, iPad, and Mac.
  2. Tapping a button sends its label as a user message and clears buttons immediately.
  3. Buttons also clear when typing manually or when a new agent turn starts.

- Verification commands:
  - `swift test --package-path swift`
  - `uv run python scripts/concerto-dev.py run-ios` (manual iPhone/iPad verification)
  - `uv run python scripts/concerto-dev.py run-debug` (manual macOS verification)
