# G1: File Items Show Diffs

Expand file items inline to show colored diff lines. Foundation for all inline diff features.

**Source:** wave/ux/02-inline-glance.md (G1)

## Problem

The original design assumed `FileEdit.diff` was already populated by agent harnesses. It's not. The Claude harness maps Edit/Write/NotebookEdit tool uses to `FileEdit` items but sets `diff: None` — the tool input has `file_path`, `old_string`, and `new_string`, but nobody synthesizes a diff from them. Codex and OpenCode harnesses do populate `diff` from their native event formats.

Without diff data flowing from the most common harness (Claude), the view layer has nothing to show.

## What's already done (this branch)

The view layer is complete and tested:

| File | Status | What |
|------|--------|------|
| `swift/Concerto/Views/DiffLinesView.swift` | Done | Standalone component: `parseDiffLines()`, colored lines, copy button, accessibility |
| `swift/LoopflowCore/State/SessionState.swift` | Done | `projectCard()` wires `FileEdit.diff` → `TranscriptItemCard.detail` |
| `swift/Concerto/Views/WaveSessionView.swift` | Done | Routes `.file` detail through `DiffLinesView` |
| `swift/ConcertoTests/DiffLinesViewTests.swift` | Done | 8 tests: all line kinds, empty input, multi-file, sequential IDs |
| `scripts/concerto-dev.py` | Done | Seeds from `wave/` dirs, background seeding, DB cleanup |

All 195 Swift tests pass. The expand chevron will appear as soon as `FileEdit.diff` is non-nil.

## What's left: populate `diff` in the Claude harness

### Approach: synthesize unified diff from Edit tool inputs

Claude's Edit tool provides `file_path`, `old_string`, and `new_string`. The Write tool provides `file_path` and `content`. We can synthesize a diff at the mapping layer without subprocess calls.

**For Edit tool uses:**

The input JSON looks like:
```json
{
  "file_path": "src/main.rs",
  "old_string": "fn old() {\n    ...\n}",
  "new_string": "fn new() {\n    ...\n}"
}
```

Synthesize a unified diff:
```
--- a/src/main.rs
+++ b/src/main.rs
-fn old() {
-    ...
-}
+fn new() {
+    ...
+}
```

This is a simplified diff (no line numbers, no context lines, no hunk headers) — but `DiffLinesView` handles all of these gracefully. Missing hunk headers just means no `@@` lines. The coloring still works: `-` lines are orange, `+` lines are green.

**For Write tool uses:**

The input has `file_path` and `content` but no before-state. Two options:
1. Show nothing (diff stays nil) — a new file write isn't meaningfully a "diff"
2. Show all lines as additions — technically correct but noisy

Recommend option 1: Write creates files, Edit changes them. Only Edit produces meaningful diffs.

**For NotebookEdit:**

Similar to Edit — has `new_source` but the old source isn't in the input. Show nothing.

### Where to change

One file: `rust/loopflow/src/lfd/sessions/harness/claude_mapping.rs`

Modify `file_changes_from_input()` (line 328) to extract `old_string`/`new_string` from Edit inputs and format as a unified diff string:

```rust
fn file_changes_from_input(tool_name: &str, input: Option<&Value>) -> Vec<FileEdit> {
    let Some(input) = input else { return Vec::new() };
    let path = input
        .get("file_path")
        .or_else(|| input.get("notebook_path"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if path.is_empty() {
        return Vec::new();
    }

    let diff = if tool_name == "Edit" {
        synthesize_edit_diff(path, input)
    } else {
        None
    };

    vec![FileEdit {
        path: path.to_string(),
        kind: Some(tool_name.to_lowercase()),
        diff,
    }]
}

fn synthesize_edit_diff(path: &str, input: &Value) -> Option<String> {
    let old = input.get("old_string").and_then(Value::as_str)?;
    let new = input.get("new_string").and_then(Value::as_str)?;
    if old == new { return None; }

    let mut lines = Vec::new();
    lines.push(format!("--- a/{path}"));
    lines.push(format!("+++ b/{path}"));
    for line in old.lines() {
        lines.push(format!("-{line}"));
    }
    for line in new.lines() {
        lines.push(format!("+{line}"));
    }
    Some(lines.join("\n"))
}
```

### Tests to add

In `claude_mapping.rs` tests:

- `synthesize_edit_diff` produces `---`/`+++` headers and `-`/`+` lines
- Edit tool use populates `FileEdit.diff` with synthesized diff
- Write tool use leaves `FileEdit.diff` as `None`
- Empty or equal `old_string`/`new_string` produces `None`
- Multi-line old/new strings produce correct line-by-line diff

### What this doesn't give you

- **No context lines.** The synthesized diff shows only deletions and additions — no surrounding unchanged lines. Real `git diff` output includes 3 context lines. For "what changed?", this is fine. For "where in the file?", you'd want line numbers (out of scope for G1).
- **No hunk headers.** No `@@ -1,5 +1,7 @@` — we don't know line numbers from the tool input. `DiffLinesView` handles this gracefully (no hunk header = no hunk line rendered).
- **Multiple edits to the same file appear as separate items.** Claude sends each Edit as its own tool use. The view layer already handles this — `projectCard()` concatenates diffs from all `FileEdit` items in a `FileItem`.

### Alternative: run `git diff` on the lfd side

Instead of synthesizing from tool inputs, run `git diff -- <path>` after each file tool use completes. This gives real unified diffs with context, hunk headers, and correct line numbers.

**Pros:** Real diffs, not synthetic. Handles Write and NotebookEdit too (shows full file as additions on create).
**Cons:** Subprocess on every file tool completion. Needs working directory context. Race condition if multiple edits land between diff calls. More complex.

This is the right long-term approach for G2 (wave-level per-file diffs already need `git diff`). For G1 (session transcript inline), the synthetic approach is sufficient and zero-latency.

## Dev tooling (also on this branch)

`scripts/concerto-dev.py` changes:
- `run-debug` seeds waves from `wave/` subdirectories (not worktrees)
- Waves created idle — no auto-run
- Background thread waits for bundled lfd, then seeds
- DB already cleared on each `run-debug`

## File map (remaining work)

| File | Action | What |
|------|--------|------|
| `rust/loopflow/src/lfd/sessions/harness/claude_mapping.rs` | Modify | `synthesize_edit_diff()`, populate `FileEdit.diff` for Edit tool uses |

## Done when

- Everything from the previous "done when" (already passing)
- Claude Edit tool uses in session transcripts show expand chevron with colored diff
- Write/NotebookEdit tool uses show no expand chevron (no diff data — expected)
- `cargo test --all` passes (new tests for `synthesize_edit_diff`)
- Visible in Concerto: start a design session, agent edits a file, expand the file item to see green/orange diff lines

**Wave goal advanced:** "You can check 'what changed?' without leaving Concerto." (02-inline-glance)
