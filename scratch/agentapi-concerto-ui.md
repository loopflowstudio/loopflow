# Concerto Session Chat Panel

## Problem

Concerto has a chat tab that sends text and renders streaming text responses. But the session API emits a richer event model — typed items (Command, File, Message, Thought, Tool) with lifecycles, turn grouping, and diffs. The current UI ignores all of this. It also doesn't reconnect to existing sessions or let users end them.

The gap: users can't see what the agent is *doing*, only what it *says*. And if they close Concerto, the session is gone.

## Approach

Extend the existing chat architecture incrementally. Three workstreams, each independently shippable:

### 1. Parse the full event model

The Swift `AgentSessionEvent` enum and `parseSessionEvent()` only handle 6 event types. Item lifecycle events (`item_started`, `item_updated`, `item_completed`, `diff_updated`) fall into `.other` and are ignored.

Add cases for the full event model:

```swift
// New AgentSessionEvent cases
case itemStarted(turnId: String, item: SessionItem)
case itemUpdated(turnId: String, itemId: String, delta: ItemDelta)
case itemCompleted(turnId: String, item: SessionItem)
case diffUpdated(turnId: String, diff: String)
```

With supporting types:

```swift
enum SessionItemType: String {
    case command, file, message, thought, tool
}

struct SessionItem: Sendable, Hashable {
    let id: String
    let type: SessionItemType
    let label: String        // "git status", "src/main.rs", "Read", etc.
    let status: ItemStatus
    let detail: String?      // output, diff, tool output — populated on completion
}

enum ItemStatus: String, Sendable, Hashable {
    case inProgress = "in_progress"
    case completed, failed, declined
}

enum ItemDelta: Sendable, Hashable {
    case output(String)
    case planText(String)
}
```

**Key decision: flatten SessionItem.** The Rust model has per-type structs (Command with argv/cwd/exit_code, File with changes/edits, etc.). The Swift model collapses these into a single struct with `type`, `label`, `status`, and optional `detail`. This is deliberate — Concerto renders items as compact inline cards, not rich tool visualizations. The label is derived at parse time:

- Command → first element of `command` array (e.g., `"git status"`)
- File → comma-separated paths from `changes` (e.g., `"src/main.rs, tests/test.rs"`)
- Message → truncated text
- Thought → truncated text
- Tool → tool name (e.g., `"Read"`, `"Write"`)

### 2. Typed item rendering in the transcript

Replace `[ChatMessage]` with `[TranscriptEntry]`:

```swift
enum TranscriptEntry: Identifiable {
    case message(ChatMessage)
    case item(TranscriptItem)

    var id: UUID { ... }
}

struct TranscriptItem: Identifiable, Equatable {
    let id: UUID
    let turnId: String
    let item: SessionItem
    let timestamp: Date
}
```

ChatState accumulates `TranscriptEntry` instead of `ChatMessage`. The event loop maps:

| Event | TranscriptEntry |
|-------|-----------------|
| textDelta | Append to / create `.message(assistant)` |
| itemStarted | Append `.item(...)` with in_progress status |
| itemUpdated | Update matching item's detail (append output delta) |
| itemCompleted | Replace matching item with completed version |
| error | Append `.message(error)` |
| diffUpdated | Update a turn-level diff summary (optional, rendered if present) |

**Rendering.** Each `TranscriptItem` renders as a compact inline card:

```
┌─────────────────────────────────────┐
│ ▶ ⌘  git status                    │   ← Command (in_progress)
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│ ✓ ⌘  git status                 2s │   ← Command (completed)
│   M src/main.rs                     │   ← output (collapsed by default)
│   M tests/test.rs                   │
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│ ✓ 📄 src/api/handler.rs   modify   │   ← File (completed)
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│ ▶ 🔧 Read                          │   ← Tool (in_progress)
└─────────────────────────────────────┘
```

Icons by type: `⌘` Command, `📄` File, `💬` Message, `💭` Thought, `🔧` Tool. Status: `▶` in_progress (with subtle pulse animation), `✓` completed, `✗` failed, `⊘` declined.

Thoughts render dimmed (secondary text color, smaller font). They're useful context but shouldn't dominate the transcript.

Command output is collapsed by default. A disclosure triangle expands it. This keeps the transcript scannable when commands produce large output.

### 3. Session lifecycle (End + Reconnect)

**End button.** Add to the chat view header (next to the tab picker, or in the composer area). Calls `stopSession()`, marks session as ended, shows "Session ended" in transcript.

```swift
// In WaveChatView or ChatState
func endSession() async {
    guard let sessionId else { return }
    _ = try? await waveService.stopSession(sessionId)
    messages.append(.message(ChatMessage(role: .system, content: "Session ended")))
    turnState = .idle
    self.sessionId = nil
}
```

Add `.system` to `ChatRole` for lifecycle messages (session started, session ended). Render in muted style, centered.

**Reconnect.** Persist `sessionId` per wave using `UserDefaults`:

```swift
private var sessionIdKey: String { "chatSession.\(waveId)" }

private func persistSessionId(_ id: String?) {
    if let id { UserDefaults.standard.set(id, forKey: sessionIdKey) }
    else { UserDefaults.standard.removeObject(forKey: sessionIdKey) }
}
```

On `ChatState` init (or first access), check for a stored session ID:

```swift
func reconnectIfNeeded() async {
    guard sessionId == nil else { return }
    guard let stored = UserDefaults.standard.string(forKey: sessionIdKey) else { return }

    // Verify session is still active
    guard let session = try? await waveService.getSession(stored),
          session.status == "active" else {
        persistSessionId(nil)
        return
    }

    sessionId = stored
    // Replay all events to rebuild transcript
    await replayEvents()
}
```

`replayEvents()` calls `streamSessionEvents(afterSeq: nil)` — the SSE endpoint replays all persisted events, then continues with live events. The event loop processes them identically, rebuilding the full transcript state. No special replay logic needed.

**When to reconnect:** On `.task` in WaveChatView. This fires when the view appears (including after Concerto restart). If a session is found, the transcript populates immediately from replay.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Turn-grouped layout (sections per turn, items nested) | Better visual hierarchy, shows conversation structure | Over-built for v1. The flat transcript is simpler and the wave item says "minimal." Can add turn headers later if needed. |
| Two-column layout (text left, activity right) | Separates conversation from tool activity | Against "no advanced layout beyond existing Concerto patterns." Also overkill — most turns interleave text and items naturally. |
| Rich item visualization (syntax-highlighted diffs, terminal emulation per command) | Detailed tool inspection | Explicitly out of scope per wave item. The inline card approach gives enough to distinguish types without building rich viewers. |
| Full Rust-matching Swift item model (per-type structs with all fields) | Preserves all data from API | YAGNI. Concerto needs label + status + optional detail. Keeping the full model means maintaining parity with Rust types for no UI benefit. Expand later if rich visualization is added. |
| Add list-sessions API endpoint for reconnect | Cleaner discovery of active sessions | Backend change for a client need. Persisting session ID client-side is simpler and sufficient for single-session-per-wave. |

## Key decisions

1. **Flat SessionItem model over per-type structs.** "No need for rich tool visualization — just enough to distinguish item types visually" (wave principle). A unified struct with `type`/`label`/`status`/`detail` covers all rendering needs. Parse-time label derivation means the UI never needs to know about `Vec<FileEdit>` or `command: Vec<String>`.

2. **Inline transcript over grouped/columnar layout.** "Minimal chat panel" (wave principle). Items appear in-line with text messages, in event order. This matches how Claude Code and Codex present their output — a single chronological stream.

3. **Client-side session persistence over API discovery.** Avoids backend changes. UserDefaults keyed by waveId is sufficient for the single-session-per-wave invariant.

4. **Replay via SSE, not separate reconstruction.** The `GET /sessions/{id}/events` endpoint replays all persisted events then continues live. We process replayed events through the same event loop that handles live events. No separate "rebuild state from events" path.

5. **Command output collapsed by default.** Agent sessions generate verbose command output. Showing it inline would bury the conversation. Collapsed with disclosure keeps the transcript scannable.

## Scope

**In scope:**
- Parse item_started, item_updated, item_completed, diff_updated events in Swift
- SessionItem model (flattened) in LoopflowCore
- TranscriptEntry model replacing ChatMessage array
- Inline item card rendering (icon, label, status, collapsible detail)
- End button calling DELETE /sessions/{id}
- Session reconnect via persisted sessionId + event replay
- System-role messages for lifecycle events
- Thought rendering (dimmed/secondary)

**Out of scope:**
- Rich tool visualization (file diffs, terminal emulation)
- Multi-session views or session picker
- Turn-level grouping / section headers
- Provider selection UI (hardcoded to "claude")
- DiffUpdated rendering (render if available, but not a core feature)
- Backend API changes

## Done when

```bash
# Swift tests pass
swift test --package-path swift
cd swift && xcodegen generate && xcodebuild test \
  -project LoopflowSwift.xcodeproj -scheme Concerto \
  -destination 'platform=macOS'

# And manually verified:
# 1. Start a session, send input, see text + items streaming in transcript
# 2. Items show correct icons and status transitions
# 3. Command output is collapsible
# 4. End button stops the session
# 5. Close and reopen Concerto → transcript rebuilds from replay
```
