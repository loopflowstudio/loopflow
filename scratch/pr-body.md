IDE skill launches could omit Wave documents, while assembled prompts could repeat the same source. This PR preserves Wave context in IDE launches, removes duplicate document delivery, and teaches `realign` to bring shared lessons from child memories into the selected parent.

<!-- loopflow:task-pr-context:start -->
> [!NOTE]
> **Task:** [Audit context allocation and subwave memory composition · LOO-331](https://linear.app/loopflow/issue/LOO-331/audit-context-allocation-and-subwave-memory-composition)
> **PR lifecycle:** Merging PR 1 completes the Task.
<!-- loopflow:task-pr-context:end -->

## What changes

- Deduplicate documents across requested context and changed files using canonical paths and matching contents, including symlink aliases. Keep distinct memory files separate even when their text matches.
- Remove redundant builtin operating guidance while preserving customized instructions, and record why each duplicate was removed.
- Use the assembled prompt for IDE launches containing Wave documents, and attribute `GOAL.md` to its document rather than also to the Wave marker.
- Instruct `realign` to discover immediate-child memories, including scopes without goals or registry entries, promote shared lessons, retain local details, and report unread coverage.

## Checks

The watched head `24ee5f5d2856b43a96be1d190f631664a919f92c` passed 2,026 Rust tests and failed the ancestor-context test. Reproduced locally: that test skipped the delivery deduplication step after gathering documents. The repair applies that existing production step and retains the single-copy and ancestor-order assertions. Review confirmed production launch paths already deduplicate; no production change was needed. TESTING.md now requires the complete context suite alongside delivery checks.

Local repair verification passed:
- `cargo test -p loopflow --test context_tests` — 17 passed.
- `cargo test -p loopflow --lib context_delivery` — 4 passed.
- `cargo fmt`.
- `cargo clippy --all-targets -- -D warnings`.
- `git diff --check`.

Earlier focused IDE delivery checks and prompt goldens passed. The full Rust suite remains owned by CI; no new full-suite local pass is claimed.

## Try it

Suggested walkthrough, not yet observed: run `lf --wave <parent> realign` where a child directory contains a useful `MEMORY.md`. Review whether the agent reads that memory and promotes a shared lesson into the parent while retaining child-specific details. The shipped guidance is covered by prompt fixtures; useful curation still needs observation.