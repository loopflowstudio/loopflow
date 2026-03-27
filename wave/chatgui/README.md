# chatgui

Native SwiftUI chat UI in Concerto — a first-class conversation experience for users who don't want a terminal.

## Vision

Some users want a chat interface, full stop. The terminal-first approach serves power users well, but a native chat UI widens the audience and gives Concerto a surface it fully controls. The infrastructure exists — streaming, transcript, tool cards, voice input, quote-reply — but the experience feels rough. Performance issues mask what's already a capable prototype.

The path isn't "build a chat UI" — it's "make the existing chat UI feel like a product."

### Not here

- Replacing the terminal experience — both coexist
- Building a general-purpose chat client — this serves loopflow agent sessions
- Feature parity with Claude's web UI — competitive on feel, not on feature count

## Goals

1. Streaming feels instant and smooth — no jank, no layout jumps, no animation conflicts
2. Rich content rendering — markdown, syntax highlighting, diffs
3. Conversation history — browse past sessions, resume
4. Composer upgrades — file drop, slash commands, context awareness

## Risks

- SwiftUI's observation model makes per-token performance hard to get right. The current architecture couples transcript mutation to full view re-evaluation. Fixing this may require restructuring SessionState.
- Markdown/syntax highlighting libraries for SwiftUI are immature. May need to build or heavily customize.
- Scope creep — "polish" is unbounded. Each stage needs a clear "done when."

## Metrics

- Frame drops during streaming: target 0 dropped frames at 30 tokens/sec with 100-entry transcript
- Time to first token visible: < 50ms from SSE receipt
- Cold launch to interactive: < 2s
