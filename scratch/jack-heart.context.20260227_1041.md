# Sprint 01: Audit Breakdown

**Finish line:** `lf implement` shows separate token rows for scratch, wave, and docs.

Split the "docs" line in the token audit header into separate line items.

## What to build

The `format_context_header()` audit display shows scratch/, wave/, summaries, and repo root docs as one merged "docs" line. Break them apart so you can see what's eating tokens.

## Data structures

```rust
// Add to DocumentSource enum in prompt.rs
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum DocumentSource {
    Step,
    Direction,
    Diff,
    RepoDoc,
    Scratch,  // NEW — was tagged as RepoDoc
    Wave,
    WaveMemory,
    Summary,
    Area,
    Clipboard,
}
```

## Key functions

```rust
// prompt.rs — tag scratch docs correctly
fn gather_scratch_docs(repo_root: &Path) -> Result<Vec<Document>, CoreError> {
    // Change DocumentSource::RepoDoc → DocumentSource::Scratch
    gather_md_files(&scratch_dir, &mut docs, DocumentSource::Scratch)?;
    ...
}

// output.rs — separate rows
fn format_context_header(breakdown: &ContextBreakdown, budget: usize) -> String {
    let scratch_tokens = breakdown.source_tokens(DocumentSource::Scratch);
    let wave_tokens = breakdown.source_tokens(DocumentSource::Wave)
        + breakdown.source_tokens(DocumentSource::Summary);
    let docs_tokens = breakdown.source_tokens(DocumentSource::RepoDoc);

    // Row order: step, direction, system, scratch, wave, diff, docs, area, ...
    // Only show rows with tokens > 0
    ...
}

// prompt.rs — budget priority update
// Drop order (first dropped): area → wave memory → summaries → repo docs → scratch → ...
// Scratch has higher priority than repo docs (drop repo docs first)
```

## Constraints

- `DocumentSource` is a HashMap key — needs Hash, Eq (already derived)
- Golden prompt tests in `cargo test -p loopflow golden_prompt` may assert on doc sources
- Parity tests `uv run pytest tests/parity/test_prompt_parity.py` may need updating
- The prompt XML structure is unchanged — scratch docs still render inside `<lf:docs>`
- `ContextBreakdown.doc_count` currently counts all docs — split into `scratch_count`, `wave_count`, `doc_count` or track per-source

## Done when

```bash
# In this repo with scratch/ content:
lf implement 2>&1 | grep -E 'scratch|wave|docs'
# Should show separate rows for each

cargo test -p loopflow
cargo clippy -- -D warnings
```
