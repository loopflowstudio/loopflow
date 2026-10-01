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

The installation compiler repair passed `task-installation` on watched head `4ed06e30012e2dadeaf1e380c8744326f80c5865`. That head's remaining `rust-test` failure was the taskless batch-launch test reading usage from `events.jsonl` before the asynchronous recorder drained. Completion waits at most 250 ms for telemetry.

This repair removes redundant event-log assertions from the launch test. It retains provider execution, retry count and identity, manifest and terminal receipts, and the absence of planning Tasks. Existing recorder tests cover usage and account events; production recorder behavior is unchanged. TESTING.md now explains the boundary and how to reproduce it.

Local verification:
- Reproduced the exact missing-usage assertion by temporarily delaying usage recording by one second; the revised launch test passed with the same delay. Removed the instrumentation before final checks.
- `cargo nextest run -p loopflow --lib -E 'test(lf::commands::run::tests::) | test(session_record::tests::) | test(engine::stream::tests::)' --no-fail-fast -j 4`: all 95 selected tests passed.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed with Rust/Clippy 1.98.
- `git diff --check`: passed.

Review: inspected the full branch diff and the repair. The launch proof now uses synchronous evidence for its assertions; no retry, sleep, timeout increase, or production behavior change is included. New-head CI remains pending publication.

## Try it

Suggested walkthrough, not performed: run `realign` for a parent Wave whose child memory contains a broadly useful lesson. Review the resulting edits: the parent should gain the shared lesson, child-specific detail should stay in the child, and any unread memories should be identified.