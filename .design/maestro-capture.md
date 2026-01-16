# Maestro: Screenshot Capture for LLM Review

**Status**: Spec

Keyboard shortcut to capture Maestro's current window state for LLM-assisted UX review.

## Behavior

**Shortcut**: `Cmd+Shift+S` (mirrors "Save screenshot" convention)

**On trigger**:
1. Capture the key window (or all Maestro windows?)
2. Save to `<repo>/.design/screenshots/maestro-<timestamp>.png`
3. Brief visual feedback (flash, sound, or subtle toast)
4. No modal, no file picker - just capture and continue

**Save location**:
- If a repo is open: `<repo>/.design/screenshots/`
- If no repo: `~/Desktop/` or `~/.lf/screenshots/`
- Create directory if needed

**Filename**: `maestro-YYYYMMDD-HHMMSS.png`

## Implementation Notes

```swift
// Capture key window
func captureForReview() {
    guard let window = NSApp.keyWindow else { return }

    let windowID = CGWindowID(window.windowNumber)
    guard let image = CGWindowListCreateImage(
        .null,
        .optionIncludingWindow,
        windowID,
        [.boundsIgnoreFraming, .bestResolution]
    ) else { return }

    let bitmap = NSBitmapImageRep(cgImage: image)
    guard let data = bitmap.representation(using: .png, properties: [:]) else { return }

    let url = screenshotURL()
    try? data.write(to: url)

    // Visual feedback
    NSSound.beep()  // or custom sound, or window flash
}

func screenshotURL() -> URL {
    let dir = repoRoot?.appendingPathComponent(".design/screenshots")
           ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Desktop")
    try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)

    let timestamp = ISO8601DateFormatter().string(from: Date())
        .replacingOccurrences(of: ":", with: "")
        .prefix(15)  // YYYYMMDDTHHMMSS
    return dir.appendingPathComponent("maestro-\(timestamp).png")
}
```

Register shortcut:
```swift
// In AppDelegate or App struct
KeyboardShortcuts.onKeyUp(for: .captureForReview) {
    captureForReview()
}

// Or via NSMenuItem with keyEquivalent
let captureItem = NSMenuItem(title: "Capture for Review", action: #selector(captureForReview), keyEquivalent: "S")
captureItem.keyEquivalentModifierMask = [.command, .shift]
```

## Usage Flow

```bash
# In Maestro: Cmd+Shift+S (captures current state)
# In Maestro: make changes
# In Maestro: Cmd+Shift+S (captures new state)

# In terminal:
lf ux-audit -x .design/screenshots/
```

## References

- Linear: Heavy keyboard shortcut usage, Cmd+K palette
- Figma: Single-key shortcuts in context, Cmd+/ search
- macOS: Cmd+Shift+3/4/5 for system screenshots

## Future

- Capture all windows vs key window
- Capture with annotation overlay (show what changed)
- Auto-capture on state transitions (for debugging flows)
- Integrate with `lf` to auto-trigger review after N captures
