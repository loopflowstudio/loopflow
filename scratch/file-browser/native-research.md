# Native file surface research

2026-09-28 · research requested by Jack Heart; recommendation, not a benchmark.

Use SwiftUI navigation with a small AppKit `NSTextView` wrapper. Start with
plain scratch editing and a read-only unified diff rendered as one selectable
attributed text document. Keep native undo, find, input methods and accessibility.
Avoid one SwiftUI text view per patch line, which complicates continuous selection.

| Option | Assessment |
| --- | --- |
| NSTextView | Best initial fit; no editor dependency. Explicitly choose TextKit 2 and avoid accessing `layoutManager`, which can trigger TextKit 1 fallback. |
| CodeEditTextView | MIT; evaluate if native layout proves inadequate. Designed for code and large documents, but lacks full system text parity, including RTL. |
| STTextView | Strong native feature set, but currently GPLv3/commercial. Requires a licensing decision for this MIT repository. |
| CodeEditSourceEditor | Larger editor feature set and dependencies; its README says it is not production-ready. |
| CodeMirror 6 in WKWebView | Mature unified/split diff building blocks, but adds JS bridging and focus ownership. Merge input requires old/new content, beyond the current patch DTO. |
| Terminal browser | Adds executable and keybinding expectations and task-specific navigation integration. Weak fit for the requested quiet native surface. |

Primary sources:

- [Apple: What's new in TextKit and text views](https://developer.apple.com/videos/play/wwdc2022/10090/)
- [CodeEditTextView](https://github.com/CodeEditApp/CodeEditTextView)
- [STTextView](https://github.com/krzyzanowskim/STTextView)
- [CodeEditSourceEditor](https://github.com/CodeEditApp/CodeEditSourceEditor)
- [CodeMirror merge](https://github.com/codemirror/merge)
- [Neon](https://github.com/slsrepo/Neon), a possible later incremental highlighting layer.

## Native constraints to resolve after mock review

- Retain document buffers outside frequent SwiftUI updates. Do not replace text
  on each keystroke or automatically reload a dirty buffer.
- Clean buffers may follow agent edits. Dirty buffers must retain local edits
  and disclose external changes before any replacement.
- Never save a truncated preview. Keep the existing 1 MB bound initially and
  show binary/truncation states explicitly.
- Load files and parse patches off the main actor. Measure first-file latency,
  typing with Ghostty alive, large patches, long lines and switching documents.
- Ghostty already requests first responder only on focus changes; preserve
  that behavior while switching between browser, editor and terminal.
- Existing Git reads compare against the immutable recorded base. Parent/HEAD
  needs real backend semantics, including matching file-list membership.
- Integrate beside retained Sessions; replacing the legacy Task workspace
  sheet preview alone does not deliver the proposed placement.
