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

With supporting types in **LoopflowCore** that mirror Rust:

```swift
enum ItemStatus: String, Sendable, Hashable {
    case inProgress = "in_progress"
    case completed, failed, declined
}

struct FileEdit: Sendable, Hashable {
    let path: String
    let kind: String?
    let diff: String?
}

struct CommandItem: Sendable, Hashable {
    let id: String
    let command: [String]
    let cwd: String
    let status: ItemStatus
    let output: String?
    let exitCode: Int?
    let durationMs: UInt64?
}

struct FileItem: Sendable, Hashable { ... }
struct MessageItem: Sendable, Hashable { ... }
struct ThoughtItem: Sendable, Hashable { ... }
struct ToolItem: Sendable, Hashable { ... }

enum SessionItem: Sendable, Hashable {
    case command(CommandItem)
    case file(FileItem)
    case message(MessageItem)
    case thought(ThoughtItem)
    case tool(ToolItem)
    case unknown(type: String, payload: JSONValue) // forward-compatible
}

enum ItemDelta: Sendable, Hashable {
    case output(content: String)
    case planText(content: String)
    case unknown(type: String, payload: JSONValue)
}
```

**Key decision: mirror in shared model, flatten at the UI boundary.** LoopflowCore should keep full typed parity with Rust for correctness, debugging, and future features. Concerto can still render compact cards by projecting typed items into a lightweight display model.

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
    let itemId: String
    let card: TranscriptItemCard
    let timestamp: Date
}

enum SessionItemType {
    case command, file, message, thought, tool, unknown
}

struct TranscriptItemCard: Equatable {
    let type: SessionItemType
    let label: String
    let status: ItemStatus?
    let detail: String?
}
```

ChatState accumulates `TranscriptEntry` instead of `ChatMessage` and maintains a canonical typed item cache keyed by `item.id`:

```swift
var itemsById: [String: SessionItem] = [:]
```

The event loop maps:

| Event | TranscriptEntry |
|-------|-----------------|
| textDelta | Append to / create `.message(assistant)` |
| itemStarted | Store typed item in `itemsById`; append `.item(...)` using projected card |
| itemUpdated | Update `itemsById[itemId]`; recompute projected card for matching transcript item |
| itemCompleted | Replace `itemsById[item.id]`; recompute projected card with terminal status |
| error | Append `.message(error)` |
| diffUpdated | Update a turn-level diff summary (optional, rendered if present) |

Projection rules for `TranscriptItemCard`:

- Command → label from joined argv (`"git status"`), detail from `output`
- File → label from changed paths (`"src/main.rs, tests/test.rs"`)
- Message/Thought → truncated text
- Tool → label from `name`, detail from `output`

**Rendering.** Each projected `TranscriptItemCard` renders as a compact inline card:

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
    transcript.append(.message(ChatMessage(role: .system, content: "Session ended")))
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

### 4. Minimal robust reducer and stream lifecycle rules

Prioritize simple rules that are easy to reason about and sufficient for v1 reliability.

#### 4.1 Event ordering + dedupe

Track `lastAppliedSeq` in ChatState and apply these rules:

```swift
if let seq = envelope.seq, seq <= lastAppliedSeq {
    return // duplicate or stale event
}
if let seq = envelope.seq {
    lastAppliedSeq = seq
}
reduce(envelope.event)
```

- Replay and live tail use one reducer path.
- Duplicate delivery is safe (idempotent).
- Stale/out-of-order events are ignored by `seq`.

#### 4.2 Stable identity for items

Never key transcript item updates by local UUID. Key by server `item.id`.

```swift
var itemsById: [String: SessionItem] = [:]              // canonical typed state
var itemEntryIdByItemId: [String: UUID] = [:]           // stable transcript row identity
```

Reducer behavior:

- `itemStarted`:
  - If `item.id` unseen: create one transcript entry and record `itemEntryIdByItemId[item.id]`.
  - If already seen: upsert typed state, do **not** append duplicate entry.
- `itemUpdated`:
  - If `item.id` known: apply delta and update existing entry in place.
  - If unknown: ignore (best-effort; later `itemStarted`/`itemCompleted` re-syncs state).
- `itemCompleted`:
  - Upsert final typed item.
  - If no entry exists yet, create one (handles completed-before-start edge case).
  - Recompute projected card with terminal status.

#### 4.3 Output/detail memory bounds

`command.output` / `tool.output` can grow indefinitely; cap detail payloads.

```swift
let detailLimit = 16_000
```

- Keep at most `detailLimit` characters in UI state per item.
- When overflow happens, keep the most recent content and append `…truncated`.
- Projection indicates truncation when applied.

This keeps long sessions responsive and avoids unbounded memory use.

#### 4.4 Reconnect/send race policy

Use a minimal explicit stream phase:

```swift
enum StreamPhase { case idle, reconnecting, live, ending }
```

- During `.reconnecting`, disable Send and show a lightweight “Reconnecting…” system status.
- `send(_:)` while reconnecting is rejected locally (no network call) with a transient message.
- Transition to `.live` only after replay has caught up and live stream is attached.

This avoids mixed old/new turn state and duplicate stream tasks.

#### 4.5 Task lifecycle + cancellation

ChatState owns exactly one stream task:

```swift
var streamTask: Task<Void, Never>?
```

- Before starting replay/live stream, cancel previous task.
- `endSession()` cancels stream task first, then calls `stopSession`.
- On view disappear, cancel stream task; on reappear, reconnect/replay from persisted session.
- Treat `CancellationError` as normal shutdown (not user-visible error).

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Turn-grouped layout (sections per turn, items nested) | Better visual hierarchy, shows conversation structure | Over-built for v1. The flat transcript is simpler and the wave item says "minimal." Can add turn headers later if needed. |
| Two-column layout (text left, activity right) | Separates conversation from tool activity | Against "no advanced layout beyond existing Concerto patterns." Also overkill — most turns interleave text and items naturally. |
| Rich item visualization (syntax-highlighted diffs, terminal emulation per command) | Detailed tool inspection | Explicitly out of scope per wave item. The inline card approach gives enough to distinguish types without building rich viewers. |
| Flatten SessionItem in LoopflowCore | Simpler short-term model | Loses typed fidelity and makes richer UI/debugging harder later. Better split: typed core model + flattened UI projection. |
| Add list-sessions API endpoint for reconnect | Cleaner discovery of active sessions | Backend change for a client need. Persisting session ID client-side is simpler and sufficient for single-session-per-wave. |

## Key decisions

1. **Typed parity in LoopflowCore; flatten only in Concerto.** Keep the shared data model aligned with Rust (`SessionItem`, `ItemDelta`, typed payload structs). Derive compact render cards in ChatState/View code.

2. **Inline transcript over grouped/columnar layout.** "Minimal chat panel" (wave principle). Items appear in-line with text messages, in event order. This matches how Claude Code and Codex present their output — a single chronological stream.

3. **Client-side session persistence over API discovery.** Avoids backend changes. UserDefaults keyed by waveId is sufficient for the single-session-per-wave invariant.

4. **Replay via SSE, not separate reconstruction.** The `GET /sessions/{id}/events` endpoint replays all persisted events then continues live. We process replayed events through the same event loop that handles live events. No separate "rebuild state from events" path.

5. **Strict reducer invariants (seq dedupe + item-id upsert).** Stability over convenience. Idempotent reduction is required for replay/live correctness.

6. **Bounded detail storage.** Keep transcript useful without allowing command/tool output to grow without limit.

7. **Single-owner stream task model.** One stream task per ChatState prevents ghost updates and lifecycle leaks.

## Scope

**In scope:**
- Parse item_started, item_updated, item_completed, diff_updated events in Swift
- Rust-mirrored SessionItem/ItemDelta model in LoopflowCore
- TranscriptEntry model replacing ChatMessage array
- TranscriptItemCard projection layer for compact rendering in Concerto
- Inline item card rendering (icon, label, status, collapsible detail)
- End button calling DELETE /sessions/{id}
- Session reconnect via persisted sessionId + event replay
- System-role messages for lifecycle events
- Thought rendering (dimmed/secondary)
- Explicit reducer rules for dedupe/out-of-order events
- Stable item identity mapping (`item.id` → transcript entry)
- Detail truncation/cap policy for command/tool output
- Stream phase/state machine for reconnect + send gating
- Stream task cancellation with single-task ownership

**Out of scope:**
- Rich tool visualization (file diffs, terminal emulation)
- Multi-session views or session picker
- Turn-level grouping / section headers
- Provider selection UI (hardcoded to "claude")
- DiffUpdated rendering (render if available, but not a core feature)
- Backend API changes

## Test plan additions

Add focused tests for reducer robustness and lifecycle safety:

1. **Replay dedupe**
   - Given duplicate seq events, transcript remains unchanged after first apply.
2. **Out-of-order handling**
   - `itemUpdated` before `itemStarted` is ignored and does not crash or duplicate entries.
3. **Stable identity**
   - Multiple updates for same `item.id` update one transcript entry (no duplicate rows).
4. **Completion without start**
   - `itemCompleted` without prior start still produces one final row.
5. **Detail cap**
   - Large output is truncated and marked as truncated; memory stays bounded.
6. **Reconnect race**
   - Send while `.reconnecting` does not call `sendSessionInput`.
7. **Task cancellation**
   - Ending session or disappearing view cancels active stream task cleanly.
8. **Stale persisted session**
   - Stored session ID with non-active/404 session is cleared and does not replay.
9. **End session UX**
   - `stopSession` called once, stream canceled, “Session ended” system message appended.

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
# 6. Duplicate/out-of-order item events do not create duplicate rows
# 7. Reconnect disables send until live stream is ready
# 8. Very large command output is truncated and UI remains responsive
```
