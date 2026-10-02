# Complete a Task from its working conversation

Design review notes · LOO-367 · 2026-10-02

## Feedback and decision

Jack Heart confirmed that the conversation doing the work can complete its Task.
The [working design](jack-heart/start-and-finish-tasks-without.md) now includes
that behavior in the interaction, completion contract and acceptance proof.
The unrelated color-scheme message was withdrawn and contributes no requirement.

## Consequences and evidence

The requesting conversation remains available to report the result and retains
its history. Its own current turn cannot be an unfinished-work prerequisite to
the Task transition. Other unfinished work and unresolved delivery retain their
protections. Completing a Task does not settle a Flow or authorize process control.

Source inspection of
[execution blockers and cleanup](../rust/loopflow/src/ops/task/lifecycle.rs)
found that the current Exec is exempted, while a non-managed Session's pending
turn can still block. Cleanup uses the same associated-work checker. Therefore
a completion-specific allowance must not become permission to delete the active
conversation's checkout or weaken abandonment checks.

The proposed implementation resolves the requesting Session and invocation from
existing authoritative evidence, retains a checkout while in use, and reports
retryable cleanup separately from Task completion. Membership or ancestry alone
does not authorize ignoring another process. No new workflow or force switch is
needed. These mechanics are proposals, not implemented behavior.

## Remaining work and proof

No further product question is outstanding. The existing full implementation
scope remains in the working design, including optional placement, shared Flow
execution, narrow provider confirmation and shared CLI/Desktop operations.
The next useful implementation work is to separate admission from placement,
then carry the same Task through binding and completion without delivery objects.

Add a real nested completion-command proof from a running bound conversation,
including its ability to report success afterward. With settled delivery and an
allocated checkout, prove completion retains files until safe cleanup. Retry after
a lost response preserves the original summary and one terminal event. Other
live/unknown work, pending review and open PRs continue to refuse completion.

Check: source inspection and `git diff --check` — pass; design-only review, no
behavioral tests run. No implementation, navigation decision or full design
acceptance is claimed.
