# Terminal Embedding Research

## Goal

Embed terminal output in Maestro to eliminate context-switching when running Claude Code. Don't compete with terminal products—integrate them.

## Options

### 1. Warp — Not Viable

Warp is closed source with no embedding API. Their Agent API is for remote task execution, not UI embedding. Licensing prohibits embedding.

### 2. Ghostty (libghostty) — Future Option

Ghostty's core is a C-ABI compatible library (`libghostty`) that can theoretically be embedded:

```
libghostty (C API) → Swift bindings → NSView → SwiftUI
```

**Status**: Alpha. API not stable. Production release expected ~mid 2026.

**Pros**: GPU acceleration, excellent performance, MIT licensed, Ghostty's own macOS app uses this architecture.

**Cons**: No stable API yet, limited embedding documentation, requires C interop.

**Recommendation**: Monitor for stable release. Not ready for production use today.

### 3. SwiftTerm — Ready Now

Pure Swift terminal emulator used in production apps (Secure Shellfish, CodeEdit, La Terminal).

```swift
import SwiftTerm

struct TerminalView: NSViewRepresentable {
    func makeNSView(context: Context) -> LocalProcessTerminalView {
        let view = LocalProcessTerminalView()
        // spawn Claude Code process with PTY
        return view
    }
}
```

**Pros**: Production-ready, stable API, pure Swift, MIT licensed, handles PTY complexity.

**Cons**: No GPU acceleration, VT100/Xterm emulation only (sufficient for Claude Code).

## Recommendation

**Short-term**: Use SwiftTerm. It's proven, embeddable, and ships today.

**Long-term**: Evaluate libghostty when API stabilizes for better performance.

## Architecture

```
Claude Code CLI (subprocess)
    ↓ PTY
SwiftTerm (LocalProcessTerminalView)
    ↓ VT100 rendering
NSViewRepresentable wrapper
    ↓
SwiftUI ResultsPanel
```

Key components:
1. **Process spawning** — Launch `claude` with PTY attached
2. **Terminal emulation** — SwiftTerm parses VT100 sequences
3. **SwiftUI integration** — NSViewRepresentable wraps the terminal view
4. **Input handling** — Forward keystrokes to PTY (if interactive mode)

## Open Questions

1. Should the embedded terminal be read-only (auto mode) or interactive?
2. How to handle terminal resize when ResultsPanel changes size?
3. Should we keep the "open in external terminal" option as fallback?

## Next Steps

1. Add SwiftTerm dependency to Maestro
2. Create `EmbeddedTerminalView` wrapper
3. Modify `TerminalLauncher` to optionally render in-app
4. Add toggle in UI to choose embedded vs external
