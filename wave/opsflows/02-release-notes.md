# 02: Release notes that tell a story

**Finish line:** `lf ops release patch` produces narrative release notes with thematic sections, prose connecting related changes, and voice consistent with previous notes. Not a bullet list.

## Verified design

All claims confirmed against the codebase. Ready for `lf implement`.

### 5 touchpoints

All in `rust/loopflow/src/ops/release.rs` and `rust/loopflow/src/engine/builtins/ops/release_notes.md`. ~100-150 LOC.

1. **`MergedPr` / `GhMergedPr` structs** — add `additions: u64`, `deletions: u64`, `changed_files: u64` fields
2. **`list_merged_prs()`** — add `additions,deletions,changedFiles` to the `--json` field list
3. **`format_pr_for_prompt()`** — remove the `max_chars = 400` body truncation, include diff stats in output (`+45 -12, 3 files`)
4. **`generate_release_notes()`** — read `RELEASE_NOTES.md` from `repo` path (if exists), append as "Previous release notes" section in prompt. No signature change needed — `repo: &Path` already available.
5. **`release_notes.md` builtin** — rewrite with narrative-first guidance: find the story, use release-specific theme names (not "Improvements"/"Bug fixes"), prose paragraphs per theme, detail bullets for scanners, opening that answers "why upgrade?"

### Key decisions

- **Full bodies, no truncation.** PR bodies are typically 0-1000 chars. Simpler than raising the limit. If prolific releases (50+ PRs) cause issues, that's a release frequency problem.
- **Diff stats from `gh pr list`**, not `git diff --stat`. `additions`/`deletions`/`changedFiles` are first-class `gh pr list --json` fields. No extra API calls.
- **Previous notes from disk.** `repo: &Path` is already available. One `fs::read_to_string`. No signature change, no caller changes.
- **Agent reads diffs selectively.** Prompt instructs `git diff` for 100+ line PRs rather than feeding all diffs upfront. Selective reading gives context without flooding tokens.
- **Theme names from changes, not a template.** Prompt says "not 'Improvements' or 'Bug fixes'" and gives examples like "Sandbox execution", "Release workflow".

### Prompt structure

The rewritten `release_notes.md` builtin instructs the agent to:
1. Read PR list, note which are large (high diff stats) vs small
2. Find connections — PRs that are part of the same theme
3. For large changes (100+ lines), `git diff` to understand what really happened
4. Write: opening (2-3 sentences) → thematic sections (prose + bullets) → small changes grouped at end

Rust appends after the template: previous release notes (if any), release context (version, prev tag, target), and formatted PR list with full bodies and diff stats.

## Risks

- **Prompt quality is empirical.** The template reads well but output quality depends on the agent. Ship it, evaluate on the next real release, iterate.
- **Large PR bodies in prolific releases.** 50+ PRs with full bodies could push prompt size. Acceptable risk — release frequency is the real fix.

## Done when

```bash
lf ops release patch
# RELEASE_NOTES.md has:
# - Narrative opening (2-3 sentences, not a list)
# - Thematic sections with release-specific names
# - Prose paragraphs connecting related changes
# - Detail bullets under each theme for scanners
# - Voice consistent with previous release notes
```

Rust tests pass (`cargo test --all`). `cargo fmt` and `cargo clippy -- -D warnings` clean.
