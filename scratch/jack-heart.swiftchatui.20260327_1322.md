# swiftchatui: Streaming Performance + Open in Ghostty

## Problem

Two things prevent the chat UI from feeling like a product:

1. **Streaming jank.** Every SSE token triggers three O(n) scans of the transcript, re-parses all visible message content, and rebuilds the entire group list. At 20+ tokens/sec with 50+ entries, frames drop. Users see stutter where they should see flow.

2. **No quick shell access.** Users working with waves need a terminal at the repo path. The pieces exist — TmuxSession, TmuxSessionRegistry, TerminalLauncher, embedded GhosttyTerminalView — but there's no "open a shell here" button, and Ghostty isn't a launch target despite being the embedded terminal engine.

Both block the chatgui and chattui waves from delivering on their core promises: "streaming feels instant" (chatgui goal 1) and "external terminal as the default launch target" (chattui goal 1).

---

## Part 1: Streaming Performance

### Approach

Move all derived transcript state out of the view's computed properties and into SessionState, where it's updated incrementally on mutation rather than recomputed on render. This is the bold choice — the design doc suggests caching in `@State` with `onChange`, but that's a bandaid. The mutation site knows exactly what changed; the view doesn't.

#### Step 0: Verify @Observable array diffing behavior

Before building the incremental state machine, prove the core assumption: that SwiftUI's `@Observable` tracking + `ForEach` over an `Equatable` array actually skips re-rendering unchanged rows on in-place mutation.

Build a minimal test harness:

1. An `@Observable` class with a `var items: [Item]` where `Item` is `Identifiable + Equatable`
2. A `ForEach` over `items`, where each row logs when its `body` runs (via `Self._printChanges()` or a render counter)
3. A button that mutates only the last element in-place (simulating a streaming delta)
4. A button that appends a new element

Run it and observe:
- **In-place mutation of last element:** Does only the last row's body re-evaluate? Or do all rows re-render?
- **Append:** Does only the new row render, or does the full list rebuild?
- **Does `Equatable` conformance matter?** Try with and without — does SwiftUI use it to skip equal rows?

If SwiftUI re-renders all rows regardless, the incremental update in SessionState still helps (no O(n) computed properties), but we'd also need `EquatableView` wrappers on `MessageRow` / `ToolRunView` to get the row-level skip. Better to know upfront than discover in Instruments after the fact.

This harness can live in a throwaway SwiftUI preview or a scratch Xcode project — it doesn't need to be committed.

#### What changes in SessionState

##### Funnel all transcript mutations through methods

`transcript` is currently mutated directly at ~8 call sites (lines 480, 500, 505, 511, 615, 620, 656, 685). Rather than adding `updateDerivedState(change:)` calls at each site, make `transcript` private and expose three mutation methods:

```swift
private var transcript: [TranscriptEntry] = []

func appendTranscriptEntry(_ entry: TranscriptEntry) {
    transcript.append(entry)
    transcriptIndexById[entry.id] = transcript.count - 1
    updateDerivedState(.append(entry))
}

func updateTranscriptEntry(id: UUID, transform: (TranscriptEntry) -> TranscriptEntry) {
    guard let index = transcriptIndexById[id] else { return }
    transcript[index] = transform(transcript[index])
    updateDerivedState(.inPlaceUpdate(index))
}

func setTranscript(_ entries: [TranscriptEntry]) {
    transcript = entries
    rebuildTranscriptIndex()
    updateDerivedState(.fullRecompute)
}
```

Every mutation goes through one of these three methods. The derived state update is guaranteed — no call site can forget it. The reverse index (`transcriptIndexById`) is maintained in the same place.

This changes SessionState's internal API but doesn't change its public surface for views — views already read transcript, they don't mutate it. The refactor is contained within SessionState.

##### New derived state

SessionState gets three new properties alongside `transcript`:

```swift
// SessionState.swift — new derived state
private(set) var groupedTranscript: [TranscriptGroup] = []
private(set) var latestAssistantMessageId: UUID? = nil
private(set) var timestampLabels: [UUID: String] = [:]
```

`updateDerivedState` does targeted updates based on the change type:

- **On append:** Extend the last group or add a new one. Update `latestAssistantMessageId` only if the appended entry is an assistant message. Recompute timestamp labels only for the new entry (check gap against previous).
- **On in-place update (streaming delta):** No group structure changes. No timestamp changes. Only `latestAssistantMessageId` might need updating (and usually doesn't — same message, same id). This is the hot path and it does O(1) work.
- **On bulk set:** Full recompute. Only happens on session load, not during streaming.

The `TranscriptGroup` enum moves from `WaveSessionView` (where it's `private`) to `SessionState.swift` as a public type with `Equatable` conformance.

#### What changes in WaveSessionView

The view reads `state.groupedTranscript`, `state.latestAssistantMessageId`, `state.timestampLabels` instead of computing them. The three computed properties are deleted.

`ForEach(state.groupedTranscript)` now benefits from `Equatable` — SwiftUI diffs the list and only re-renders changed groups. During streaming, only the last group's content changes.

#### parseMessageSegments memoization

`MessageRow` caches its parsed segments in `@State`:

```swift
@State private var cachedSegments: [MessageSegment] = []
@State private var cachedContentLength: Int = 0

// In body:
let segments: [MessageSegment] = {
    if message.content.count != cachedContentLength {
        cachedContentLength = message.content.count
        cachedSegments = parseMessageSegments(message.content)
    }
    return cachedSegments
}()
```

Content length as staleness check is cheap and correct — streaming only appends text, so length change means new content. This avoids re-parsing the 49 unchanged messages on every delta.

#### appendAssistantDelta O(n) fix

`appendAssistantDelta` does `transcript.firstIndex(where: { $0.id == entryId })` on every delta. Add a reverse index:

```swift
private var transcriptIndexById: [UUID: Int] = [:]
```

Updated on every append/remove. Turns the lookup from O(n) to O(1). This is the innermost hot path — called 20-30 times per second during streaming.

#### Scroll animation

Replace the bare `withAnimation` at line 71:

```swift
.onChange(of: state.transcript.count) { _, _ in
    guard isNearBottom, let last = state.transcript.last else { return }
    withAnimation(DesignAnimation.fast(reduceMotion)) {
        proxy.scrollTo(last.id, anchor: .bottom)
    }
}
```

Note: `onChange(of: transcript.count)` only fires on appends (new messages/items), not on in-place delta mutations. This is correct — we scroll on new entries, not on every token. The fix is just replacing the bare animation.

#### StreamingCursorView identity

Pin the cursor view with a stable `.id()` so SwiftUI doesn't recreate it when the parent group re-renders:

```swift
if showStreamingCursor {
    StreamingCursorView()
        .id("streaming-cursor-\(message.id)")
}
```

The `message.id` is stable across deltas (same UUID). The animation state (`@State private var isVisible`) survives because the view identity doesn't change.

### Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Cache in `@State` + `onChange` in view | Simpler diff, keeps SessionState minimal | `onChange` still fires per-delta and the view still evaluates three closures to update caches. Pushes bookkeeping into the view layer where it doesn't belong. |
| Dedicated StreamingViewModel | Clean separation, testable | Adds a new layer between SessionState and the view. Over-engineering for what's fundamentally a "compute once, read many" problem. |
| `EquatableView` wrapper on MessageRow | Skips re-render of unchanged rows | Doesn't fix the O(n) computations that run before ForEach even diffing. Helps at the row level but misses the forest. |
| Throttle transcript mutations | Cap update rate at e.g. 15/sec | Adds visible latency. Users notice when tokens appear in bursts rather than flowing. Wrong tradeoff for a native app. |

### Key decisions

1. **Derived state lives in SessionState, not the view.** The mutation site knows what changed — the view shouldn't have to figure it out. This is a minor expansion of SessionState's responsibility but dramatically simplifies the view and makes the hot path O(1).

2. **Content length as cache key for parseMessageSegments.** Not a hash, not a version counter. Length is free to compute and sufficient because streaming only appends. If we later support editing messages, we'd need a content hash — but we don't.

3. **TranscriptGroup becomes public and Equatable.** It was `private` in the view. Making it public in SessionState is the right call because it's derived data that belongs with the source data.

4. **No throttling.** Native apps should feel native. 30 tokens/sec at 60fps means ~2 tokens per frame — if each token does O(1) work, there's no need to throttle.

---

## Part 2: Open in Ghostty

### Approach

Add Ghostty as a first-class terminal launcher. One new enum case, one launch function, one button wired into the existing command palette and workspace sidebar.

#### TerminalApp enum

```swift
public enum TerminalApp: String, Sendable, CaseIterable {
    case ghostty
    case warp
    case iterm
    case terminal
    case kitty
}
```

Ghostty goes first — it's the default. The `displayName` is "Ghostty".

#### TerminalLauncher.launchGhostty

Unlike Warp/iTerm/Terminal.app which use AppleScript, Ghostty launches via direct Process execution (like Kitty):

```swift
case .ghostty:
    let ghosttyPath = "/Applications/Ghostty.app/Contents/MacOS/ghostty"
    var arguments = ["--working-directory=\(path.path)"]
    if let command {
        arguments.append("--command=\(command)")
    }
    let process = Process()
    process.executableURL = URL(fileURLWithPath: ghosttyPath)
    process.arguments = arguments
    try process.run()
```

For tmux-backed sessions, the command is `tmux attach-session -t <session-name>`.

#### Workspace "Open Terminal" button

The command palette action at `ContentView.swift:168` already opens the user's preferred terminal. Change the default from `.warp` to `.ghostty`. Add an "Open Terminal" toolbar button to `TerminalWorkspaceView`'s context sidebar (alongside the existing "Open in Cursor" and "Reveal in Finder" buttons at line ~320):

```swift
Button {
    let sessionName = "lf-\(waveId)-shell"
    Task {
        let tmux = TmuxSession(sessionName: sessionName, worktreePath: cwd, registry: .shared)
        await tmux.ensureBaseSession()
        try terminalLauncher.launchTerminal(.ghostty, at: URL(fileURLWithPath: cwd),
            command: "tmux attach-session -t \(sessionName)")
    }
} label: {
    Label("Open Terminal", systemImage: "terminal")
}
```

This creates the tmux session if needed, then opens Ghostty attached to it. If the session already exists, `ensureBaseSession` is a no-op and Ghostty attaches to the running session.

#### "Open Internally" — same tmux session, embedded

The embedded path already works via `SessionTerminalSurface` which calls `repoState.attachTerminalSession()` → gets `TerminalConnectionInfo` → builds `tmux attach-session -t` argv → passes to `GhosttyTerminalView`. No new code needed for the embedded attach. The button just needs to create/select a terminal session that points at the same tmux session name:

```swift
Button {
    let session = TerminalSession(
        id: "shell-\(waveId)",
        waveId: waveId,
        step: "shell",
        agent: "interactive",
        cwd: cwd,
        argv: ["tmux", "attach-session", "-t", "lf-\(waveId)-shell"],
        source: "user_shell"
    )
    terminalStore.upsert(session, select: true)
} label: {
    Label("Open Internally", systemImage: "rectangle.inset.filled")
}
```

#### Session lifecycle

- **Create:** `TmuxSession.ensureBaseSession()` creates the tmux session, registers in `TmuxSessionRegistry`
- **External attach:** Ghostty window opens with `tmux attach -t`. Closing the window detaches but doesn't kill the session.
- **Internal attach:** `GhosttyTerminalView` embeds with the same tmux attach argv. Navigating away from the terminal pane detaches.
- **Cleanup:** `TmuxSessionRegistry.killAllSynchronously()` on app quit (already wired up).
- **Reattach:** Opening either button again finds the existing tmux session and attaches.

### Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Warp as default | More common terminal | Warp uses AppleScript keystroke injection — fragile, can't auto-run commands reliably. Ghostty has a proper CLI and is already the embedded engine. |
| Terminal.app fallback | No install dependency | Terminal.app can't attach to tmux via CLI args cleanly. Also doesn't match the quality bar. |
| No tmux backing | Simpler | Closing the window kills the shell. Users lose state. tmux is the whole point — persistent sessions you can reattach from either surface. |
| Auto-detect installed terminal | Flexible | Over-engineering. Ghostty is bundled via libghostty. Make it the default, let users change in preferences. |

### Key decisions

1. **Ghostty is the default terminal.** It's what Concerto embeds. It has a proper CLI. One terminal, two modes (embedded and standalone).

2. **tmux session naming: `lf-<waveId>-shell`.** Matches existing convention in TmuxSession. The `-shell` suffix distinguishes user shells from agent sessions (`lf-<waveId>-<step>`).

3. **No new UI for terminal preference.** The existing command palette and sidebar buttons use `TerminalApp` which is already surfaced in preferences. Adding `.ghostty` to the enum is sufficient.

---

## Implementation order

Four chunks. Part 2 and Part 1a are independent — run as parallel subagents.

| Chunk | What | Files | Depends on |
|-------|------|-------|------------|
| **Part 2** | Ghostty launcher + workspace buttons | AppPreferences.swift, TerminalLauncher.swift, TerminalWorkspaceView.swift | — |
| **Part 1a** | Mutation funneling + reverse index | SessionState.swift | — |
| **Step 0** | @Observable array diffing harness | throwaway preview | Part 1a (needs the new types) |
| **Part 1b** | Derived state in SessionState, TranscriptGroup moved + Equatable, view reads state | SessionState.swift, WaveSessionView.swift | Part 1a, Step 0 results |
| **Part 1c** | parseMessageSegments memo, scroll animation, cursor identity | MessageRow.swift, WaveSessionView.swift | Part 1b |

Part 2 and Part 1a touch zero overlapping files — safe to parallelize. Part 1b depends on both 1a (mutation methods exist) and Step 0 (know whether we need EquatableView wrappers). Part 1c is view-level cleanup that layers on top.

## Scope

**In scope:**
- Part 1: Incremental derived state in SessionState, parseMessageSegments memoization, Equatable on TranscriptGroup, scroll animation fix, cursor identity pin, transcript index cache
- Part 2: Ghostty case in TerminalApp/TerminalLauncher, "Open Terminal" and "Open Internally" buttons, tmux session creation and lifecycle

**Out of scope:**
- Markdown rendering (chatgui stage 03)
- Conversation history (chatgui stage 04)
- Multi-agent dispatch (chattui stage 03)
- Reattach UI for detached sessions (chattui stage 02 — the tmux sessions survive, but the sidebar reattach buttons are a separate piece)
- Terminal preference settings UI
- Replacing Warp as default in all existing places (just add Ghostty as an option; changing default is a separate conversation)

## Done when

**Part 1:**
```bash
# Build and run Concerto
cd swift && xcodegen generate && xcodebuild build -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO
```
- Instruments Time Profiler during 100-message streaming at 30 tok/s: no frame drops
- `groupedTranscript`, `timestampLabelByMessageId`, `parseMessageSegments` absent from top 10 callers during streaming
- Scroll animation respects `reduceMotion`
- StreamingCursorView doesn't flicker on each delta
- Existing Concerto UI tests pass:
```bash
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

**Part 2:**
- "Open Terminal" on a wave workspace opens Ghostty `cd`'d to the repo, backed by tmux
- Closing the Ghostty window doesn't kill the session
- "Open Internally" attaches the same tmux session in Concerto's embedded Ghostty view
- Session appears in TmuxSessionRegistry
- Swift package tests pass: `swift test --package-path swift`

## Measure

**Part 1 — before/after with Instruments:**
- Baseline: record Time Profiler during a streaming session now. Note frame drops, top callers.
- After: same recording. Target: 0 frame drops at 30 tok/s with 100 entries. The three O(n) computed properties should not appear in the trace.

**Part 2 — functional verification:**
- Click "Open Terminal" → Ghostty window appears within 1s
- Run `tmux ls` → session `lf-<waveId>-shell` exists
- Close Ghostty → `tmux ls` still shows the session
- Click "Open Internally" → embedded terminal shows the same shell
