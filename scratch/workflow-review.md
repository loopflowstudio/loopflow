# Workflow command review · October 7, 2026

Jack Heart requested reviewing LOO-386 here, then selected pr-review. The
[walkthrough](pr-review.html) covers published PR #1492 at `7d2f0fd48`, against
`e467ea995`. The [design](align-the-data-model-and-names-after-workflows-one-more-pass.md)
remains the implementation contract. The walkthrough initially covered the published revision; subsequent feedback
produced the local naming correction recorded below.

Jack asked what TaskRun means. Source has no TaskRun record/table: the term
means one conceptual traversal attempt. TaskRunControl carries an unavailable
reason; Desktop TaskRunDraft carries only acting/error for an in-flight run or
move. Neither owns execution history. No further acceptance was given.

Observed using the branch binary in a temporary directory and private Home:
Flow and Workflow feature coexist, customize prints separate paths, malformed
Workflow source remains visibly unavailable while the Flow is unaffected, and
removing the malformed override restores the builtin. A unique Workflow-only
name fails independent execution and names the Task command for recovery.
An exploratory `lf run research` instead resolved the existing research skill;
it returned without a research question or source. That is not Workflow traversal.

The installed lf lacks the new Project commands. Branch help emitted a process
ledger warning; definition reads and customization succeeded. No live Project
selection or Task position was changed, and no native Desktop interaction was
performed. Fixture proof is not installed adoption or provider continuity.

Source review highlights a consequential inherited behavior: restart moves the
position even if a Flow is active. The Flow keeps running, but its eventual
arrival cannot advance the reset Task. Jack's reset decision did not separately
review that scenario. Project-level definition list/customize placement remains
an implementation choice within Jack's owner-scoped direction.

Check: LF_* cleared, `cargo test -p loopflow --test task_flow_launch_tests workflow_restart_keeps_the_captured_graph_and_execution_history -- --exact --nocapture` passed 1/1; HTML excerpts checked against pinned head, desktop and narrow section renders inspected with lf screenshot (file-fragment captures were blank; isolated section previews rendered correctly).

Native Desktop proof remains unperformed. Jack's subsequent delivery approval
below supersedes the earlier review hold; it does not establish native proof.

## Task run evidence naming

Jack Heart found `TaskRunGate` confusing alongside the gate skill. Rename the
input struct to `TaskRunEvidence` and its parameter to `evidence`; it contains
the facts used to derive TaskRunControl. Behavior and wire fields stay the same.
This is the agent’s concrete naming correction to Jack’s feedback.

## Delivery approval

Jack Heart accepted the naming correction and requested “ok sounds good lets
ship it.” This authorizes the authored ship edge (gate, then Task-completing
landing). It is approval of the reviewed change with TaskRunEvidence, not a
claim of performed native Desktop, live Linear mutation or provider-continuity
checks. The walkthrough remains pinned to the earlier published head; this
note records the subsequent naming change and approval.

Naming check: `cargo fmt --all`, focused Task run refusal test (1/1), and `cargo clippy --all-targets -- -D warnings` passed with LF_* cleared for test/Clippy.

## Gate review

Gate found one shared JSON fixture inventing idle execution for planning-only
Tasks, and four inline Swift fixtures still using the removed `flow` object.
Corrected the shared rows to absent execution and migrated the mock Wave screen,
RegistryQuery and Wave lens fixtures to the explicit Workflow/execution/control
fields. Removed an unused navigation-test identity left by the rename and changed
the guide's remaining “folded template” to “folded graph.” Existing producer and
consumer tests prove the repairs; the design holds the single check-result line.

The Python observation test cannot install its sandbox inside the runner's macOS
sandbox. It passed directly with its own network/write/read restrictions intact.
The initial gate run remains failed evidence; focused repairs and this bounded
fallback resolve its findings without claiming an identical-tree cached pass.
No production lifecycle, schema, retry policy or authority change was needed.
