# Unfinished interactive Sessions (LOO-372)

Jack Heart authorized this local repair on October 3, 2026 without a human review
gate. Ordinary API and Desktop navigation show unfinished interactive
conversations and current authored reviews. Explicit headless/history inspection
retains background and completed evidence. Publication and merge are not requested.

## Implemented

Desktop refresh now consumes the API default unless Show headless Sessions is
explicitly enabled. Navigation, Task materials, palette, breadcrumb choices and
counts use the same visible population. Task association remains unchanged.
`--needs-me` narrows the selected interactive mode instead of implicitly
broadening it. Explicit Session links retain their existing broader lookup.

Filtered absence no longer removes retained Session items, clears selection or
releases native surfaces. Explicit completion still removes the completed item
and fences older refreshes. Partial pages and changes of filter retain the
existing generation checks. No schema or lifecycle redesign was necessary.

## Delete — do not maintain

Removed Desktop's unconditional headless refresh and Task-membership visibility
exception, absence-based native surface release, and `--needs-me`'s implicit
mode override. Preserve attribution, Flow membership, provider processes,
review authority, history, drafts and pane layouts.

## Review and lifecycle audit

The existing completion transaction records conversation completion separately
from provider turn events. Confirmed owning-driver exits retire disposable
orphans while retaining Task, primary and review conversations. Missing or stale
evidence remains insufficient. Inspection and focused regressions found no
additional proven lifecycle-recording defect requiring a change.

Diff review caught two consequential presentation paths: filtered disappearance
previously freed retained surfaces, and Task detail/material counts did not share
navigation's membership/scope exclusions. Both are repaired. Tests cover mixed
inventory, explicit background/history access, completed-turn retention,
prepared-command retention, review completion/resume and completion racing an
older refresh. API inventory uses an isolated store and real CLI; provider review
uses a stand-in. Headless Desktop checks do not prove configured provider input
or native visual acceptance. Neither is a requested gate for this repair.

Checks: `scripts/test_desktop.sh --filter 'PodiumModelTests|SessionControlsTests|SessionsStoreTests|WorkspaceNavigationTests|WorkspaceDestinationTests|DesktopHeadlessTests|SessionRenameTests|WorktreeWorkspaceTests'` passed 92 tests; after final count cleanup `scripts/test_desktop.sh --filter 'DesktopHeadlessTests|WorkspaceNavigationTests|SessionsStoreTests|SessionControlsTests'` passed 47; `cargo test -p loopflow --test session_lifecycle_tests inventory_scopes_before_paging_and_keeps_worktree_repository_identity` and `cargo test -p loopflow --test session_lifecycle_tests review_feedback_survives_replacement_and_resumes_the_flow` passed; `cargo test -p loopflow --lib store::sqlite::session_events::tests` passed 12; `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` passed.

## Remaining work

No remaining implementation within the authorized local repair. The change is
unpublished. LOO-371's Task-opening performance work remains separate.
