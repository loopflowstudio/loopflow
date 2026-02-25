# 02: Action Buttons

Agents surface next actions as tappable buttons. Primary mobile interaction, also works on desktop.

## What to build

A `suggest_actions` tool convention that agents use to surface clickable next-action buttons. The client renders them as tappable buttons below the chat. Tapping a button sends its text as the user's next message. Mobile-first but renders on Mac too.

Stage 02 is UI/protocol convention work only. It does not include discovery (Stage 04) or multi-client transport changes (Stage 03).

## Protocol

The agent calls a tool named `suggest_actions` with structured input:

```json
{
  "actions": [
    {"label": "Land PR", "description": "Merge the PR and clean up"},
    {"label": "Run tests again", "description": "Re-run the test suite"},
    {"label": "Show me the diff"}
  ]
}
```

- `label` (required): button text, also the message sent when tapped
- `description` (optional): subtitle or tooltip

This is a regular tool_use in the session protocol. No new wire format. The client recognizes `suggest_actions` by name and renders buttons instead of the default tool card.

## System prompt (A — do first)

Add to the agent's system text via the session config message field:

```
You have a tool called `suggest_actions`. Use it to suggest 2-4 next actions
the user might want to take. Call it after completing a task, when waiting for
user input, or when presenting results. Each action should be a short phrase
that makes sense as a user message. Prefer concrete actions ("Land PR",
"Run tests") over vague ones ("Continue", "Tell me more").
```

Concerto injects this into `AgentSessionConfig.message` when creating a session. No lfd changes needed. Can experiment immediately.

## Tool registration (B — do after A works)

lfd injects a formal tool definition for `suggest_actions` into the agent's tool list:

```json
{
  "name": "suggest_actions",
  "description": "Suggest 2-4 next actions the user might want to take.",
  "input_schema": {
    "type": "object",
    "properties": {
      "actions": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "label": {"type": "string"},
            "description": {"type": "string"}
          },
          "required": ["label"]
        }
      }
    },
    "required": ["actions"]
  }
}
```

This gives schema validation and makes the tool show up in the agent's tool list natively. Requires lfd changes — session creation adds the tool definition to the provider's tool set.

## Data model

```swift
// In LoopflowCore
public struct SuggestedAction: Sendable, Hashable, Identifiable {
    public let id: UUID
    public let label: String
    public let description: String?

    public init(label: String, description: String? = nil) {
        self.id = UUID()
        self.label = label
        self.description = description
    }
}
```

## ChatState additions

```swift
// New property
var suggestedActions: [SuggestedAction] = []

// Parse from ToolItem when name == "suggest_actions"
private func parseSuggestedActions(from item: ToolItem) -> [SuggestedAction]? {
    guard item.name == "suggest_actions",
          case .object(let obj) = item.input,
          case .array(let actions) = obj["actions"] else { return nil }

    return actions.compactMap { value -> SuggestedAction? in
        guard case .object(let action) = value,
              let label = action["label"]?.stringValue else { return nil }
        return SuggestedAction(
            label: label,
            description: action["description"]?.stringValue
        )
    }
}
```

Clear actions when:
- User sends a message → `suggestedActions = []`
- Agent starts a new turn → `suggestedActions = []`
- New `suggest_actions` tool completes → replaces previous

## View

```swift
// In LoopflowCore (shared view)
public struct ActionButtonsView: View {
    let actions: [SuggestedAction]
    let onTap: (SuggestedAction) -> Void

    @Environment(\.horizontalSizeClass) private var sizeClass

    // iPhone: vertical stack, full-width buttons
    // iPad/Mac: horizontal pills, wrapping flow layout
}
```

**Integration note (from Stage 01):** Chat surfaces are platform-specific — MobileWaveDetailView on iOS, WaveChatView on macOS. ActionButtonsView lives in LoopflowCore as a shared component, but each platform's chat view embeds it. ChatState lives in `LoopflowCore/State/`, so `suggest_actions` parsing logic should stay there for both platforms.

### iPhone rendering

Action buttons render prominently above the composer:

```
┌─────────────────────────┐
│  [agent message]        │
│                         │
│  ┌───────────────────┐  │
│  │  Land PR          │  │  ← tappable, full-width
│  │  Merge and clean  │  │
│  └───────────────────┘  │
│  ┌───────────────────┐  │
│  │  Run tests again  │  │
│  └───────────────────┘  │
│  ┌───────────────────┐  │
│  │  Show me the diff │  │
│  └───────────────────┘  │
│                         │
│  [composer input]  [Send]│
└─────────────────────────┘
```

### iPad / Mac rendering

Horizontal pill buttons below the last message:

```
[Land PR]  [Run tests again]  [Show me the diff]
```

### Behavior

- Tap sends `label` as user message (same as typing it and hitting send)
- Buttons disappear after tap (or when agent starts responding)
- Buttons also disappear when user types manually
- Agent can call `suggest_actions` multiple times per conversation — latest wins
- If agent doesn't call it, no buttons shown (graceful absence)

## Constraints

- Start with A (system prompt injection) — no lfd changes needed
- B (tool registration) comes after A is working end-to-end
- Don't block on lfd changes — can test with mock data while waiting
- ActionButtonsView is a shared LoopflowCore component; platform chat views embed it rather than forking the button behavior
- ChatState parsing is shared (`LoopflowCore/State/`); both platforms consume `suggestedActions` from the same state

## Done when

- Agent calls `suggest_actions` via system prompt instructions, buttons render in MobileWaveDetailView (iOS) and WaveChatView (macOS)
- Tapping a button sends the label as the next user message
- Buttons clear on tap or when agent responds
- Works on iPhone, iPad, and Mac
