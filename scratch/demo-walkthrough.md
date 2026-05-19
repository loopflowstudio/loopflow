# Demo walkthrough — Desktop branch

Headless run: backend/Core verification ran here; the SwiftUI rendering must be
seen on a machine with a display. Both observable changes below are real and
test-backed; the UI half needs a human at a Mac.

## What's new

1. **Assistant messages render as native markdown.** Before, macOS chat showed
   raw `#`, `*`, `-` and uncolored code. Now headings are burgundy, lists
   bullet/number, **bold**/*italic*/links resolve inline, ` ```rust ` blocks
   are syntax-colored, and ` ```diff ` / ` ```patch ` blocks render through the
   real `DiffLinesView` (add/remove gutters), not a flat code box.
2. **Flow launches are lfd-owned terminal sessions.** A palette launch now
   creates a session via `POST /v0/terminal-sessions`; the flow can exit while
   the tmux pane stays attachable, and the session survives daemon/app
   restarts.

## Verified here (headless)

| Check | Command | Result |
|-------|---------|--------|
| Markdown parse + highlighter | `swift test --package-path swift --filter MarkdownBlock` | 4/4 pass |
| Wire shape (3-language) | `cargo test -p loopflow --test dto_fixtures` | 4/4 pass |
| Embedded terminal lifecycle | `uv run python scripts/verify_embedded_build_driver.py` | `OK: failed session stayed attachable` |

The terminal script proves the core promise: a palette session whose flow
*fails* still leaves an attachable tmux shell (session name
`lf-jack-heart-desktop-...`), which is what makes restart-survival work.

## Manual UI walkthrough (needs a display)

```
uv run python scripts/concerto-dev.py run-debug
```

Then:

1. Open a wave session. Send a prompt that elicits a reply with a heading, a
   bulleted list, **bold**, a link, a ` ```rust ` block, and a ` ```diff `
   block. Confirm each renders as a styled native element (burgundy heading,
   colored Rust tokens, diff gutters) — not literal markdown.
2. While the reply streams, watch for scroll/layout jank. The streaming row
   uses the cheap fence-split path (`parseStreamingMarkdownBlocks`); the rich
   parse + highlight runs once at turn completion, cached by
   `(message.id, finalLength)` in `MarkdownBlockCache`.
3. From the command palette, launch a flow into the embedded terminal pane.
   Confirm the pane binds an lfd `terminalSessionId`. Quit and reopen Concerto;
   confirm the session reattaches with output preserved.

## Notes / limits

- Streaming smoothness ("0 dropped frames at 30 tok/s, 100-entry transcript")
  is designed for via the split render path but is **not** measured in this
  headless run — needs the manual step 2 above on a real display, or the
  perf-budget test called for in the design's Measure section.
- The syntax highlighter is a heuristic tokenizer by design (small fixed
  language set, no IDE-grade accuracy). Verify Rust/Swift blocks look right by
  eye during the manual walkthrough — mis-tokenized color is worse than none.
- Out of scope on this branch: conversation history (M2) and composer file
  drop / slash commands (M3).
