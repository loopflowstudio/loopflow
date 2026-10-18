# LOO-416 handoff — October 17, 2026

Jack Heart authorized pursue through demo/review, including shared repository,
Wave and Task admission, every ensure caller, nested CLI lifetime and readiness.
No merge or auto-merge authorization. Current scope and remaining work are in
[the plan](loo-416-keep-task-sessions.md). Full prior handoff, including the
uncommitted runner recovery evidence, is preserved in
[evidence/lifecycle-handoff-before-query.txt](evidence/lifecycle-handoff-before-query.txt).

LOO-415 PR #1581 is integrated at `7bfa10fa72`, following retention checkpoint
`172ce48a39`. Admission and typed attach-only reconnect are checkpointed at
`a3e929a7ef8d614c667086af1e1107d1799c6fb1`; preserve their passing checks. The old
0.14.3 runner failed against the 0.14.4 store; recovery uses compatible installed
0.14.4. Read current execution through `lf task status LOO-416 --json`; earlier
missing exits are history, not work to replay. No competing implementation or
provider retry is authorized.

LOO-497 owns shared records/model; LOO-489 owns native presentation/save-restore.
Their committed handoff is required before durable integration. October 17 reads
confirm LOO-497 at `959c714fbb` and LOO-489 published at `4ad912f336` with a live
Flow. Jack Heart narrowed that LOO-489 slice to built-in query views, superseding
older broad persistence plans. Neither published head supplies the tab/draft
restoration contract; the remaining delivery dependency is explicit in the plan.

Accepted later tool-tab direction: each Task has one Shell tool and one Files
tool, each with ordered tabs and retained selection. ShellTab.id is the stable
TerminalIdentity.shell identity, independent of pane/focus; native incarnation is
runtime-only. Portable WorkShellTab retains optional lf_process_id and recorded
location, never liveness/launch authority. Restored command stays nil until
attach-only Process reconnection succeeds; never replay saved argv. Files tabs
share Task/path drafts; Files location belongs to the tool. Only Tasks create
shells; reconcile historical repo/Wave/Dashboard shells without silent deletion.
Close/Undo fences removed tabs. LOO-416 supplies continuity through the agreed
owner APIs; sibling Tasks retain the tab/persistence cut.
