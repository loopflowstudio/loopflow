# Folders

Unified file storage conventions for loopflow: documented folder hierarchy, `.docs/` auto-inclusion, and goal loading for autonomous agents.

## Review

**Verdict:** Ready to ship

The branch delivers what it set out to do, with clean implementation and thorough tests.

### What's implemented

1. **`.docs/` auto-inclusion** — `gather_internal_docs()` in `design.py` collects markdown files recursively. Context assembly order is `.design/` → `.docs/` → root docs. Four tests verify inclusion, ordering, and that public `docs/` stays opt-in.

2. **Goal loading** — `load_goal()` handles name lookup (`.lf/goals/{name}.md`) and explicit paths. Agent runner injects goals with `<lf:goal:{name}>` tags. Four tests cover the loading logic.

3. **`--docs` renamed to `--lfdocs`** — CLI flag and config key renamed for clarity. All four occurrences in `run.py` updated, test updated.

4. **PR title extraction fix** — `_extract_json_payload()` now starts searching for `{` after any `json` fence, avoiding false matches on `{placeholder}` patterns in prose. New test file `test_messages.py` with 7 tests.

5. **Built-in prompts updated** — `design.md`, `implement.md`, `review.md` now reference `.docs/` in their workflows.

6. **Documentation** — New `docs/storage.md` explains the folder philosophy. Config docs reorganized with context assembly section and demo gif. Public `docs/vision.md` removed (kept internal in `.docs/`).

7. **Summarize tests** — 29 new tests in `test_summarize.py` covering metadata, hashing, staleness, content gathering, and config integration.

### Style notes

The inline imports in `summarize.py` (lines 245, 277, 312, 327) avoid circular imports at module load time. The pattern is acceptable here—this is the one case where STYLE.md allows it.

## Design notes

**Ephemeral vs persistent.** `.design/` clears on merge; `.docs/` persists. This enforces: if it matters after merge, put it in `.docs/`.

**Context order.** `.design/` (ephemeral, current work) → `.docs/` (persistent internal) → root `.md` files. Most relevant context first.

**Goal injection.** Goals wrapped in `<lf:goal:{name}>` tags, visually distinct from task prompts.
