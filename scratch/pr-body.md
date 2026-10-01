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

The original watched head `789a68d4001fb3901a480e18bd2c51a781a9e100` passed the CI test jobs but failed rust-lint on Rust 1.99.0. An older local Clippy missed seven diagnostics. The repair uses the renamed atomic update method, removes single-element loops and needless borrows, and documents matching CI's toolchain and Cargo subcommand PATH.

Local verification on Rust 1.99.0:
- `cargo +stable fmt --all -- --check` passed.
- `cargo +stable clippy --all-targets -- -D warnings` passed after reproducing all seven failures.
- Materialized focused nextest run passed all 37 tests across installation validation, Task file saves, Wave relocation, and the Claude harness and recorded traces. Live subscription tests remain excluded.
- Reviewed the complete diff: validation conditions, error messages, relocation preservation, and atomic ordering remain unchanged; no lint suppressions were added.

CI must validate the newly published head.

## Try it

Suggested walkthrough, not performed: run `realign` for a parent Wave whose child memory contains a broadly useful lesson. Review the resulting edits: the parent should gain the shared lesson, child-specific detail should stay in the child, and any unread memories should be identified.