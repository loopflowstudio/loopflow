# 02: Release notes that tell a story

**Finish line:** `lf ops release` produces narrative release notes with thematic sections and prose. The LLM gets full PR context and researches actual diffs for major changes.

## What to build

**Richer input** — in `generate_release_notes()` (Rust):

- Remove 400-char truncation on PR bodies. Full bodies.
- Add diff stats per PR (files changed, +/- lines).
- Feed previous RELEASE_NOTES.md for voice continuity.
- If total context exceeds model limits, summarize per-PR with sub-agents, then synthesize.

**Better prompt** — rewrite `release_notes.md` builtin:

Output format:
1. Narrative opening — 2-3 sentences. Why upgrade?
2. Thematic sections — named after actual themes, not generic categories. Prose paragraph per theme.
3. Detail bullets under each theme — for scanners.

Prompt guidance:
- "What's the story of this release?"
- PR summaries are a table of contents — dig deeper into big changes
- Read diffs and code to understand implications
- Find connections between PRs that are part of the same story
- Theme names specific to this release
- Match voice/style of previous release notes

## What changes

```rust
// Remove truncation
// Before: body.chars().take(400).collect()
// After: full body

// Add previous notes to context
fn generate_release_notes(
    repo: &Path,
    prs: &[MergedPr],
    version: &str,
    prev_tag: &str,
    target: &ReleaseTarget,
    previous_notes: &str,  // NEW
) -> OpsResult<String>
```

Rewrite `rust/loopflow/src/engine/builtins/ops/release_notes.md` with narrative-first guidance.

## Done when

```bash
lf ops release patch
# RELEASE_NOTES.md has:
# - Narrative opening (not just listing changes)
# - Thematic sections with specific names
# - Prose connecting related changes
# - Detail bullets for scanners
```
