# Replies: Quote-Reply UX for Concerto Sessions

## Problem

In Concerto interactive sessions, LLM responses are often long and multi-part — code suggestions, explanations, alternatives, questions. Right now you reply to the *whole message*. But your reactions are positional: "yes to this, no to that, change this specific thing." You end up writing lossy prose like "yes to most of it, but not the part about X, and for Y change it to Z."

## What exists after this

Concerto's chat view supports inline quote-replies. You highlight any part of an LLM response, reply with text or a quick emoji react, queue up multiple replies as you read through, and send them all as one structured message. The LLM receives a clearly formatted response that maps your feedback to the specific parts it wrote.

## Milestone 1: Demo panel with options

Before building the real thing, build a prototyping surface inside Concerto.

### Demo panel

A `ReplyDemoView` accessible from a dev/debug menu in Concerto. It renders:

- A fake LLM response (multi-paragraph with code blocks, lists, prose)
- Multiple UX options side-by-side or switchable:
  - **Option A**: Popover style — small floating toolbar at selection (Medium-style)
  - **Option B**: Inline annotation — reply appears anchored below the highlighted text
  - Option variants for the emoji palette (small fixed set vs. picker)
- The draft tray in different states: empty, 1 item, 3 items, full with mixed types
- The assembled message preview showing what the LLM would receive

All mock data, no session wiring. The goal is to see the options and give feedback.

### Why a demo panel

The development loop is: agent writes code → launch Concerto → open demo panel → screenshot for review → iterate. No Xcode previews needed. The panel also serves as a permanent component gallery for future UX prototyping.

### `#Preview` blocks

Add `#Preview` variants on each component too (cheap, 3-4 lines). Free option value if anyone opens Xcode. But the demo panel is the primary feedback surface.

## Milestone 2: Reply implementation

Wire the chosen option into the real chat view.

### Interaction model

**Always available.** No mode toggle. Selecting text in an LLM message naturally surfaces reply options.

### Flow

1. Read through an LLM response
2. Highlight a section — a reply popover appears with:
   - Text input for a typed reply
   - Quick-react emoji buttons (👍 👎 ✏️ ❓ or similar)
3. Submit the reply/react — it gets queued in a draft tray
4. Keep reading, highlight more, reply more
5. Optionally add free-text entries (not anchored to any highlight)
6. Review the queue — reorder, edit, delete entries
7. Send — all queued items get assembled into one structured message

### Queue entry types

| Type | Trigger | Example |
|------|---------|---------|
| Quote-reply | Highlight → type text | `> use composite key` + "No, use a UUID" |
| Emoji react | Highlight → tap emoji | `> migration script` + 👍 |
| Free text | Type in queue directly | "Also, make the tests use fixtures" |

### Draft tray

Bottom strip above the normal input field:

```
┌─────────────────────────────────┐
│  LLM response                   │
│  ...highlighted section...      │
│  ...more text...                │
│                                 │
├─────────────────────────────────┤
│  ▾ 3 replies queued        Send │  ← draft tray (collapsible)
├─────────────────────────────────┤
│  Type a message...              │  ← normal input (still there)
└─────────────────────────────────┘
```

Expand to see queued items. Each shows truncated quote + reply/emoji. Can delete or edit entries.

### Assembled message format

```
> use a composite key for the junction table

No, use a UUID. Composite keys make the join queries ugly.

> here's the migration script

👍

> we should also add an index on created_at

This should be a partial index on active records only.

Also, can you make the tests use fixtures instead of inline data?
```

### Data model

```swift
enum ReplyEntry: Identifiable {
    case quoteReply(id: UUID, quoted: String, reply: String)
    case emojiReact(id: UUID, quoted: String, emoji: String)
    case freeText(id: UUID, text: String)
}

@Observable
class ReplyQueue {
    var entries: [ReplyEntry] = []

    func assembleMessage() -> String { ... }
}
```

## Platform

- Concerto only (SwiftUI)
- Desktop first, mobile also (different selection gesture: mouse highlight vs. long-press)
- Not a TUI experience

## Open questions

- Fixed small emoji palette vs. customizable?
- Visual annotations on the LLM response showing which parts have replies?
- Only latest response, or quote-reply to earlier messages too?
- Exact quoting vs. smart trimming of highlighted text?

## Done when

**Milestone 1:**
- Demo panel accessible from Concerto dev menu
- Shows at least 2 popover/annotation style options with mock data
- Draft tray visible in multiple states
- Assembled message preview renders correctly
- Screenshottable for `lf ux-review`

**Milestone 2:**
- Text selection in chat bubbles surfaces reply popover
- Emoji reacts and text replies queue in draft tray
- Free text entries can be added to queue
- Send assembles and dispatches structured message via ChatState
- Works on macOS (desktop selection)
