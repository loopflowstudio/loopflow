# 05: Prompt Pipeline

Replace stringly-typed document categories with a typed pipeline.

## What exists after this

`DocumentSource` enum replaces `category: String`. Documents gathered via `gather_documents(specs)` with explicit source specification. Formatting consolidated into `format_prompt(mode)`.

## Current state

`engine/prompt.rs` (2429 lines) uses `Document { path, content, category: String }` where category is one of: `"wave"`, `"docs"`, `"diff_files"`, `"summaries"`, `"area"`.

Gathering happens procedurally across ~500 lines: wave docs, area docs, diff, clipboard, lfdocs each gathered by separate code paths with inline filtering. `GatherContextOpts` has boolean flags (`lfdocs`, `diff_files`, `diff`, `clipboard`) that control which paths run.

`ContextBreakdown` tracks token counts per category for display. Formatting functions (`format_prompt`, `format_context_prompt`, `format_task_prompt`) handle different output modes.

## Approach

### Step 1 — DocumentSource enum

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentSource {
    Step,
    Direction,
    Wave,
    Area,
    Diff,
    Clipboard,
    RepoDoc,
    Summary,
}
```

Replace `category: String` on `Document`. Update `ContextBreakdown` to use `DocumentSource` keys.

### Step 2 — Gather specs

Replace boolean flags with explicit gather specifications:

```rust
pub struct GatherSpec {
    pub sources: Vec<DocumentSource>,
    pub repo_root: PathBuf,
    pub area: Option<String>,
    pub wave: Option<String>,
    // ...
}

pub fn gather_documents(spec: &GatherSpec) -> Vec<Document> {
    // Dispatch per source, collect results
}
```

Each source has a focused gatherer. The top-level function dispatches and collects.

### Step 3 — Format consolidation

Merge `format_prompt`, `format_context_prompt`, `format_task_prompt` into one entry point with a mode parameter. Keep thin wrappers for backwards compatibility during migration if needed.

### Step 4 — Delete string categories

Remove all `"wave"`, `"docs"`, `"area"` string literals. Category filtering uses enum matching.

## Key files

| File | Lines | What changes |
|------|-------|-------------|
| `engine/prompt.rs` | ~2429 | DocumentSource enum, gather_documents, format consolidation |
| `engine/mod.rs` | | Export new types |

## Risks

- **Prompt parity**: `cargo test -p loopflow golden_prompt` and `tests/parity/test_prompt_parity.py` verify prompt output. Any refactor must preserve exact output. Run parity tests after each step.
- **Large file**: 2400 lines in a single file. Refactoring in place is safer than splitting during this stage.

## Done when

- `Document.category` is `DocumentSource` enum, not `String`
- No string category literals in prompt assembly
- `gather_documents(spec)` replaces procedural gathering
- Formatting consolidated (one entry point, mode parameter)
- Golden prompt and parity tests pass
