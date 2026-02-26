# Session Polish: Make Concerto's Interactive Sessions Lovable

The session experience works. Now make it sing. Every rough edge filed down, every missing affordance added, every moment of "what's happening?" eliminated.

Inspired by: Claude Code's rawness and speed feel, Conductor's checkpoint/diff review, Sculptor's pairing mode and session persistence, OpenCode's terminal-native beauty.

---

## B. Session Polish (the build)

### B1. Thinking indicator with streaming feel

**Now:** `ProgressView("Thinking…")` — a generic spinner. No sense of life.

**After:** A pulsing accent bar that grows as text streams in, with phase-aware labels.

```swift
// Replace the static ProgressView with a phase-aware thinking indicator
struct ThinkingIndicator: View {
    let phase: StreamPhase
    let turnState: TurnState

    // Three states:
    // 1. "Connecting…" (awaiting session join)
    // 2. "Replaying…" (replay phase)
    // 3. Animated pulsing bar (running, no label — the animation IS the indicator)

    // The pulsing bar: 3pt accent bar (same as message accent bars)
    // that pulses opacity 0.3→1.0 on a 1.2s ease-in-out loop.
    // Respects reduceMotion — falls back to static "Thinking…" text.
}
```

**Why:** Claude Code streams to terminal — you *see* it working. The pulsing bar gives that same "alive" feeling without fake typing animation. When textDelta arrives, the bar vanishes and real content takes over seamlessly.

**Done when:** The thinking state feels alive, not stuck. No layout jumps when text starts streaming.

### B2. Markdown code blocks with syntax structure

**Now:** `AttributedString(markdown:, interpretedSyntax: .inlineOnlyPreservingWhitespace)` — inline only. Code blocks render as plain text blobs.

**After:** Parse markdown into blocks. Render fenced code blocks with monospace font, surface background, and a copy button.

```swift
// Split assistant message content into segments
enum MessageSegment {
    case text(AttributedString)      // inline markdown as before
    case codeBlock(language: String?, code: String)
}

// Parse by splitting on ``` fences
func parseMessageSegments(_ content: String) -> [MessageSegment]

// Render code blocks:
struct CodeBlockView: View {
    let language: String?
    let code: String

    // - palette.surface background
    // - CornerRadius.md
    // - Typography.code(13) — JetBrains Mono
    // - Optional language badge top-right (caption, muted)
    // - Copy button top-right (doc.on.doc icon), appears on hover
    // - Horizontal scroll for long lines (no wrapping)
    // - textSelection(.enabled)
}
```

**No syntax highlighting.** That's a separate dependency (tree-sitter, Splash, etc.) and not worth the complexity now. The monospace + surface background + copy button is the 80/20. Syntax highlighting can come later as a pure additive improvement.

**Done when:** Assistant messages with triple-backtick code blocks render in a visually distinct card with copy button. Inline markdown (bold, italic, `inline code`) continues working as before.

### B3. Copy buttons on tool output

**Now:** Tool/command output is selectable text (`textSelection(.enabled)`) but no quick copy.

**After:** Hover-revealed copy button on expanded detail sections.

```swift
// In TranscriptItemCardView, when detail is expanded:
if isExpanded, let detail = card.detail {
    ZStack(alignment: .topTrailing) {
        Text(detail)
            .font(Typography.code(12))    // was caption — code font is better for output
            .foregroundStyle(palette.textSecondary)
            .textSelection(.enabled)
            .frame(maxWidth: .infinity, alignment: .leading)

        CopyButton(text: detail)
            // doc.on.doc icon, 24x24 hit target
            // palette.textSecondary, opacity 0 → 1 on hover
            // Checkmark feedback for 1.5s after copy
    }
}
```

Also add copy button to code blocks (B2) and to user/assistant messages (long-press or hover).

**Done when:** You can copy any output, code block, or message with one click. Visual feedback confirms the copy.

### B4. Richer tool output typography

**Now:** Detail text uses `Typography.caption()` (Lato, small). Command output deserves monospace.

**After:** Tool and command detail uses `Typography.code(12)`. Thoughts stay in caption. File items show path in code font.

```swift
// In TranscriptItemCardView detail rendering:
let detailFont: Font = switch card.type {
    case .command, .tool: Typography.code(12)
    case .thought: Typography.caption()
    default: Typography.body()
}
```

Small change, big readability win for the most common expanded content.

### B5. Tighten action button responsiveness

**Now:** 300ms delay before `allowsHitTesting(true)`. This prevents accidental double-taps but makes actions feel sluggish.

**After:** Reduce to 150ms. The 300ms was likely conservative — 150ms is still enough to prevent double-taps while feeling responsive. If on macOS (not compact), drop to 100ms since accidental taps aren't an issue with mouse.

```swift
let delay: Duration = isCompact ? .milliseconds(150) : .milliseconds(100)
```

### B6. User message accent bar and visual hierarchy

**Now:** User messages have a burgundy accent bar. Assistant messages have a very faint textSecondary bar at 0.4 opacity. Both look similar at a glance.

**After:** Strengthen the visual distinction:
- User messages: burgundy accent bar (keep as-is)
- Assistant messages: **no accent bar**. They get the full width. The asymmetry makes the conversation scannable — your messages have the bar, the agent's don't.
- Error messages: keep red accent bar

This is how most chat UIs work — user messages are visually "tagged" and assistant messages flow naturally.

### B7. Streaming text cursor

**Now:** Text appears but there's no visual cue that more is coming.

**After:** While a textDelta is actively streaming (turnState == .running && the message is the latest assistant message), show a blinking cursor character at the end of the text.

```swift
// In MessageRow, for the latest assistant message while streaming:
if isStreaming {
    (Text(markdown) + Text("▊")
        .foregroundStyle(palette.textSecondary)
        .opacity(cursorOpacity))  // pulses 0→1 on 0.6s timer
}
```

Respects `reduceMotion` — shows a static `▊` instead of blinking.

**Done when:** You can see text streaming in and know more is coming. Cursor disappears when turn completes.

### B8. Timestamp improvements

**Now:** Every message shows a time-of-day timestamp. This is noisy for rapid back-and-forth.

**After:** Show relative timestamps only when there's a gap:
- First message in a turn: show time
- Messages within 60s of previous: no timestamp
- Messages after a gap: show "2m ago" or time-of-day if > 1hr ago

This declutters rapid exchanges while preserving temporal context for long sessions.

### B9. Empty state for new sessions

**Now:** New session opens to an empty transcript.

**After:** Show a subtle welcome state:

```
What would you like to work on?

[Type a message or tap a suggested action]
```

Centered, muted text. Disappears as soon as first message is sent. Uses serif italic per the design system for that editorial feel.

### B10. Keyboard shortcut for focus

**Now:** No keyboard shortcut to jump to the composer.

**After:** Pressing `/` when not already in a text field focuses the composer (same as command palette in other views). `Escape` from composer returns focus to transcript.

---

## A. Checkpoints & Revert (design thinking)

### The Conductor insight

Conductor creates a git checkpoint before every user message. Each checkpoint captures: current commit + index + worktree as a private ref. You can view the diff between any two turns and revert with one click.

### How this maps to Concerto

Concerto sessions run inside lfd-managed worktrees. The agent already commits atomically. The gap is:

1. **Turn-indexed snapshots** — lfd would need to snapshot worktree state at each turn boundary (turnStarted/turnCompleted events). This is a daemon-side feature, not just UI.

2. **Turn-by-turn diff view** — in the transcript, each turn could have a "View changes" expand that shows the git diff for that turn's work. This is a UI feature that reads from the snapshot data.

3. **Revert** — clicking revert on a turn would `git checkout` the worktree back to that turn's snapshot. This needs careful UX: what happens to the session? The transcript? Do we destroy messages after that point (Conductor's approach) or keep them visible but grayed out?

### Rough architecture

```
lfd side:
  - On turnStarted: git stash + write-tree to refs/concerto-checkpoints/<session>/<turn>
  - Expose endpoint: GET /sessions/:id/checkpoints → [{turnId, ref, timestamp}]
  - Expose endpoint: GET /sessions/:id/checkpoints/:turn/diff → unified diff
  - Expose endpoint: POST /sessions/:id/checkpoints/:turn/revert → restore worktree

Concerto side:
  - TranscriptEntry gets optional `hasCheckpoint: Bool`
  - Hovering/tapping a turn header shows "View changes" and "Revert to here"
  - Diff renders inline using a simple +/- line coloring (green/red) in code font
```

### Open questions

- Does reverting also revert the session state (tell the agent "I reverted to turn N, continue from here")? Or is it just worktree state?
- Should checkpoints be opt-in (explicit "save" button) or automatic at every turn?
- Performance: writing git trees at every turn could slow down fast iterations. Benchmark needed.

### Recommendation

Build the UI affordance first — a "View changes" button on each turn that shows the diff. This is useful even without revert. Ship it, learn from it, then add revert.

---

## C. Session persistence (already solid)

Concerto persists session IDs in UserDefaults per wave and auto-reconnects with 3-phase replay. The `streamPhase` state machine (idle → replaying → live → ending) is well-built.

What Sculptor adds that we don't have: browsable session *history* (past sessions, not just current). This would be nice but isn't blocking — the current reconnect-to-active-session pattern covers the main use case. Could be a future wave item.

---

## D. Diff-first review (design thinking)

### The Conductor insight

Conductor gives Claude a tool to read the workspace diff — full diff, per-file diff, or stat summary. Claude then comments on specific lines, not markdown tables. Users click "Review changes" and see inline annotations.

### How this maps to Concerto

We already have `.diffUpdated` events that SessionState currently ignores:

```swift
case .diffUpdated(_, _):
    return  // Currently no-op
```

### Rough architecture

```
lfd side:
  - .diffUpdated events already flow through the session stream
  - Need to enrich with structured diff data (file, hunks, +/- lines)
  - Claude's inline comments could come as a new event type: .diffComment(file, line, comment)

Concerto side:
  - New TranscriptEntry variant: .diff(DiffView)
  - DiffView shows files changed with expandable hunks
  - Green/red line coloring for additions/deletions
  - Inline comment bubbles at specific lines
  - "Review changes" action button that triggers Claude to read and comment on the diff
```

### The simpler version

Before building full inline diff review, the 80/20 is:

1. **Render file items with actual diffs** — FileItem already has `changes: [FileChange]` with `path`, `kind`, and `diff` fields. Currently shown as a label "path1, path2". Instead, render the diff.
2. **Color-coded diff lines** — green for additions, red for deletions, in code font. Same rendering could be reused for checkpoints (A).

### Done when

File items in the transcript show actual diffs with +/- coloring. Users can see what changed without leaving Concerto.

---

## Implementation order

For the B items (the build), priority by impact-to-effort ratio:

1. **B2 + B3** — Code blocks + copy buttons (biggest single UX gap)
2. **B1 + B7** — Thinking indicator + streaming cursor (makes it feel alive)
3. **B4** — Monospace tool output (tiny change, immediate readability win)
4. **B6** — Remove assistant accent bar (visual clarity)
5. **B8** — Smart timestamps (declutter)
6. **B5** — Tighten action button delay (responsiveness)
7. **B9** — Empty state (first impression)
8. **B10** — Keyboard focus (power user)

This is roughly ~800-1000 LOC of focused UI work. Fits in one PR.

---

## Constraints

- No new dependencies (no tree-sitter, no third-party markdown parsers). Use what Swift/SwiftUI gives us.
- All animations must respect `reduceMotion`.
- All interactive elements need `accessibilityLabel`.
- Touch targets >= 44pt on compact, >= 24pt on regular.
- JetBrains Mono for all code/output. Lato for body. Cormorant Garamond reserved for editorial moments (B9 empty state).
