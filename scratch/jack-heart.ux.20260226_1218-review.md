# G1: File Items Show Diffs — Review

## What was implemented

Two layers of work, both on this branch:

**View layer (Swift):** `DiffLinesView` — standalone component that parses unified diff strings and renders colored lines (green additions, orange deletions, muted context/headers). Self-contained copy button, accessibility labels, reduce-motion support. `SessionState.projectCard()` wires `FileEdit.diff` through `TranscriptItemCard.detail`. `TranscriptItemCardView` routes `.file` detail to `DiffLinesView` instead of the generic monospace renderer.

**Data layer (Rust):** `synthesize_edit_diff()` in the Claude harness mapping layer. Extracts `old_string`/`new_string` from Edit tool inputs and formats as simplified unified diff (file headers + deletion/addition lines). Write and NotebookEdit tool uses leave `diff` as `None` — no before-state available.

**Dev tooling (Python):** `concerto-dev.py` seeds waves from `wave/` subdirectories instead of worktrees. Background seeding for bundled lfd. DB cleanup on each `run-debug`.

## Key choices

- **Synthetic diff, not `git diff`.** The Claude Edit tool input has `old_string`/`new_string` — enough to synthesize deletion/addition lines without subprocesses. No context lines or hunk headers, but `DiffLinesView` handles their absence gracefully. Real `git diff` is the right long-term approach for G2.

- **Concatenate diffs, don't restructure.** Multiple `FileEdit` diffs join as one string via `\n`. The unified diff `---`/`+++` headers serve as visual separators. Keeps `TranscriptItemCard` generic.

- **Self-contained `DiffLinesView`.** Owns its copy button rather than relying on the parent `CopyButton`. G2 will embed `DiffLinesView` in `WaveDetailPanel` where there's no parent copy infrastructure.

- **Write/NotebookEdit → no diff.** Write creates files (no before-state), NotebookEdit doesn't include old source. Only Edit has meaningful before/after data.

## How it fits together

```
Claude harness   →   synthesize_edit_diff()   →   FileEdit.diff: Some("--- a/...")
                                                        ↓
SessionState.projectCard()                    →   TranscriptItemCard.detail
                                                        ↓
TranscriptItemCardView                        →   DiffLinesView(diff:)
                                                        ↓
                                                  Colored lines + copy button
```

Data flows from Rust → Swift models → SwiftUI views. Each layer is independently testable.

## Risks and bottlenecks

- **`replace_all` Edit.** When Claude's Edit uses `replace_all: true`, the diff shows one replacement, but all occurrences changed. Inherent limitation of synthesizing from tool input.

- **Large diffs.** No truncation — a user who expands wants to see it. Long diffs scroll. If this proves unwieldy in practice, truncation is a simple follow-up.

- **No context lines.** Synthetic diffs show only what changed, not surrounding code. Sufficient for "what changed?" but not "where in the file?". G2's `git diff` approach solves this.

## What's not included

- Syntax highlighting within diff lines
- Line numbers
- Word-level inline diff highlighting
- G2 wave diff stat expand (separate item)
- Truncation / "show more" for long diffs
- FileEdit `kind` badge (create/edit/delete indicator)

## Test coverage

| Suite | Tests | Status |
|-------|-------|--------|
| Rust `claude_mapping` | 22 (6 new: synthesize format, equal/empty, multiline, edit→diff, write→none, edit-no-old→none) | Pass |
| Swift `DiffLinesViewTests` | 8 (all new: empty, addition, deletion, hunk, header, context, mixed, multi-file, sequential IDs) | Pass |
| Swift full suite | 195 | Pass |
| `cargo fmt --check` | — | Pass |
| `cargo clippy -- -D warnings` | — | Pass |

## Wave alignment

**Advances:** "You can check 'what changed?' without leaving Concerto." (wave/ux/02-inline-glance, G1)

G1 is the foundation — once `FileEdit.diff` is populated, the view layer renders it. The remaining items (G2–G5) build on this.
