# Ghostty Key Handling + Session Cleanup

Two bugs found during demo of the terminal embedding branch.

## Bug 1: Modifier keys not forwarded to tmux

**Symptom:** `Ctrl-b` (tmux prefix) moves cursor backward instead of activating tmux prefix. `Ctrl-b "` prints literal `"~`. Tmux is attached (confirmed via `tmux list-sessions`) but never sees the prefix keystroke.

**Root cause:** The `keyDown` refactor on this branch simplified the `ghostty_surface_key` call path but dropped the `key.text` assignment for non-control printable keys. Two interacting problems:

### Problem A: `insertText` swallows control characters

`interpretKeyEvents` calls `insertText` for `Ctrl-b`, passing `\x02` (stripped of modifier). `_didInsertText` gets set to `true`, so `keyDown` returns early. The control character goes through `ghostty_surface_text` as raw text — tmux never sees `Ctrl+b` as a key event with the control modifier.

**Fix:** In `insertText`, detect single control characters (unicode scalar < 0x20) and bail early *without* setting `_didInsertText`. Let the keystroke fall through to `ghostty_surface_key` which preserves the modifier.

```swift
// In insertText(_:replacementRange:), before setting _didInsertText:
if let scalar = text.unicodeScalars.first, text.unicodeScalars.count == 1, scalar.value < 0x20 {
    return
}
```

### Problem B: `key.text` dropped for all non-IME keys

The old code (on main) had three branches in the fallthrough path:

```swift
// OLD (main)
if mods.contains(.control) || mods.contains(.command) {
    _ = ghostty_surface_key(surface, key)           // no text — correct
} else if let chars = event.characters, !chars.isEmpty {
    chars.withCString { textPtr in
        key.text = textPtr
        _ = ghostty_surface_key(surface, key)       // text attached — needed
    }
} else {
    _ = ghostty_surface_key(surface, key)
}
```

The refactored code removed all three branches and always sends `key.text = nil`:

```swift
// NEW (this branch)
let key = translateKey(event)
_ = ghostty_surface_key(surface, key)               // no text ever
```

Ghostty needs `key.text` for printable characters that bypass IME (any key that `interpretKeyEvents` doesn't handle via `insertText`). Without it, Ghostty falls back to keycode-only decoding which may produce wrong characters or `~` artifacts.

**Fix:** Restore the three-branch logic. Control/command combos: no text (correct, Ghostty uses keycode + mods). Printable keys: attach `event.characters` as `key.text`. Everything else: keycode only.

### Combined fix for keyDown

```swift
override func keyDown(with event: NSEvent) {
    guard let surface else {
        super.keyDown(with: event)
        return
    }

    _didInsertText = false
    interpretKeyEvents([event])

    if _didInsertText { return }

    if _markedRange.location == NSNotFound {
        var key = translateKey(event)
        let mods = event.modifierFlags
        if mods.contains(.control) || mods.contains(.command) {
            _ = ghostty_surface_key(surface, key)
        } else if let chars = event.characters, !chars.isEmpty {
            chars.withCString { textPtr in
                key.text = textPtr
                _ = ghostty_surface_key(surface, key)
            }
        } else {
            _ = ghostty_surface_key(surface, key)
        }
    }
}
```

### Combined fix for insertText

```swift
func insertText(_ string: Any, replacementRange: NSRange) {
    guard let surface else { return }

    let text: String
    if let attrString = string as? NSAttributedString {
        text = attrString.string
    } else if let str = string as? String {
        text = str
    } else {
        return
    }

    // Control characters (Ctrl-b, Ctrl-c, etc.) arrive stripped of modifier.
    // Let them fall through to ghostty_surface_key so the modifier is preserved.
    if let scalar = text.unicodeScalars.first, text.unicodeScalars.count == 1, scalar.value < 0x20 {
        return
    }

    _didInsertText = true
    unmarkText()

    text.withCString { ptr in
        ghostty_surface_text(surface, ptr, UInt(text.utf8.count))
    }
}
```

**File:** `swift/Concerto/Platform/macOS/Services/Ghostty/GhosttyTerminalView.swift`

---

## Bug 2: Abandoned tmux sessions

**Symptom:** `tmux list-sessions` shows multiple `lf-*` sessions accumulating. Each wave selection creates a new tmux session (keyed by wave UUID) but sessions are never cleaned up when:
- The wave view is deselected
- Concerto quits
- The wave is deleted

**Current state:** `TmuxSession` in `swift/Concerto/Platform/macOS/Services/TmuxSession.swift` creates sessions via `tmux new-session -d` in `ensureBaseSession()` but has no teardown path. `GhosttyMetalView.deinit` calls `teardownSurface` and unregisters from `GhosttyManager`, but nobody kills the tmux session.

**Fix approach:**

1. Add `kill()` method to `TmuxSession`:
   ```swift
   func kill() async {
       do {
           try await run("tmux", "kill-session", "-t", sessionName)
       } catch {
           // Session already gone — fine
       }
   }
   ```

2. Call it from `MultiplexerStore` or `RepoState` when:
   - A wave is deselected (debounced — don't kill if reselected quickly)
   - A wave is deleted
   - App terminates (register via `NSApplication.willTerminateNotification`)

3. For app termination, a synchronous cleanup pass:
   ```swift
   // In ConcertoApp or AppDelegate
   NotificationCenter.default.addObserver(forName: NSApplication.willTerminateNotification, ...) { _ in
       // Kill all lf-* sessions
       Process.launchedProcess(launchPath: "/usr/bin/env",
           arguments: ["tmux", "kill-server", "-t", "lf-*"])  // or iterate known sessions
   }
   ```

   Or more targeted: track active session names in `MultiplexerStore` and kill each one.

**Files:**
- `swift/Concerto/Platform/macOS/Services/TmuxSession.swift` — add `kill()`
- `swift/LoopflowCore/State/MultiplexerStore.swift` — track sessions, cleanup on deselect
- `swift/Concerto/ConcertoApp.swift` — cleanup on termination
