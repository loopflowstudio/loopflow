# LOO-370 execution decisions — October 4

Jack Heart authorized autonomous implementation, verification and landing. The
previous design-review hold is superseded; no additional product approval is
required for reversible source design choices. Jack suggested `sessions/` for
the retired `runs/` layout; implementation must resolve the actual capture owner
without treating an artifact key as a Session ID.

Use the existing Task and checkout. Prove preservation and interrupted conversion
in isolation. Do not migrate the installed Home or interrupt live conversations.
The earlier offline-window proposal remains a deployment design choice to test,
not evidence that a configured maintenance window has been performed or approved.

Current design: [Finish the Run cutover](finish-removing-the-retired-run.md).

## Runtime recovery blocker — October 4

The supported restart saved managed code Flow
`b2476b7b-a8b2-4b61-9c04-d146871206d3`, replacing feature, but startup timed out.
Fresh Task status shows idle implement with no claimed worker. A same-Flow start
using an isolated tmux directory was rejected because the replaced review
`task_5341076d02ed439798b9349bf7c5cdb6:69217c2a-11d9-4bd0-831f-ad95786d4e72:feature:review_kickoff:0`
awaits completion and Execs `02c0220f-01f6-4d49-9c14-ca54b66fb3c6` and
`9f7a4496-bddb-47f4-8f06-77e0c8393327` have live or unresolved execution.
The supported session-complete command rejects that exact review as no longer
waiting. Both Exec inspections lack terminal outcomes. No competing worker or
raw database rewrite was used. Autonomous development/landing authorization
remains valid; repair supported retirement/admission reconciliation before
continuing the same saved code Flow. Implementation has not started.
