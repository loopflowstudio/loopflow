Wave context could be repeated in assembled prompts or omitted from IDE skill launches. This PR delivers the complete selected Wave goal once and teaches `realign` to consult child memories when updating a parent’s guidance.

<!-- loopflow:task-pr-context:start -->
> [!NOTE]
> **Task:** [Audit context allocation and subwave memory composition · LOO-331](https://linear.app/loopflow/issue/LOO-331/audit-context-allocation-and-subwave-memory-composition)
> **PR lifecycle:** Merging PR 1 completes the Task.
<!-- loopflow:task-pr-context:end -->

## What changes

- Use assembled prompts for IDE launches with Wave documents so the provider receives their contents.
- Deduplicate repeated document sources across reference documents and changed files, including symlink aliases. Preserve distinct memory files even when their text matches, and avoid repeating builtin operating guidance.
- Record why documents were deduplicated and attribute `GOAL.md` to its delivered document rather than also to the Wave seed.
- Instruct `realign` to discover immediate-child memories, including directories without goals or registry entries, promote shared lessons into the selected parent, and keep local details in their owning files. Unread coverage must remain explicit; this is agent guidance, not automatic memory merging.

## Checks

The watched head `29bb67bffc7b7188fe28ad67232fc5a46e674c3a` passed the other CI jobs but failed `task-installation`: its pinned Rust 1.89 image could not compile `AtomicUsize::try_update`. The installation harness now pulls `rust:bookworm` to follow stable Rust and logs its compiler version. TESTING.md requires the installation proof when adopting newer standard-library APIs.

Local verification:
- Reproduced the unstable API error with Rust 1.89 in Docker.
- `uv run python scripts/test_task_installation.py` passed with Rust 1.98.1: populated-planning migration and all six managed Task installation proofs.
- `uv run ruff check scripts/test_task_installation.py` passed.
- `uv run ruff format --check scripts/test_task_installation.py` passed.
- `git diff --check` passed.

Reviewed the complete diff. This repair changes only the installation compiler selection, version logging, and verification guidance; all proof assertions remain intact. New-head CI remains pending publication.

## Try it

Suggested walkthrough, not performed: run `realign` for a parent Wave whose child memory contains a broadly useful lesson. Review the resulting edits: the parent should gain the shared lesson, child-specific detail should stay in the child, and any unread memories should be identified.