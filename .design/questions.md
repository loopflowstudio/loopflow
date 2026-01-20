# Open Questions

## Platform-independent clipboard image handling

The clipboard image code uses AppleScript (`osascript`) which is macOS-only. Options for cross-platform support:

1. **Pillow with ImageGrab** — Works on macOS/Windows/Linux but adds a heavy dependency (Pillow)
2. **pngpaste** — macOS only, requires separate brew install
3. **pyobjc** — macOS only, requires additional dependency

Given the README explicitly lists macOS as a requirement and `pbpaste` is already macOS-specific, platform independence would require a larger architectural decision. The current AppleScript approach works and was simplified to use one subprocess call instead of two.
