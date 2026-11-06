# LOO-498 — native workspace launch

Jack Heart's November 5 correction supersedes the passive-viewer design: create a
native Rex tab in the Task worktree and run ordinary `lf resume`. The same command
construction applies to Herdr and cmux. No custom observer or input-transfer UX.
Jack requested dramatic simplification after repeated demos opened his active
Task and rendered overlapping text. Do not open, close, resume, or modify his
running conversation or native panes while implementing or testing this change.
Use isolated fixtures for validation; future hands-on demos need separate work.

## Keep

- Native host detection, fresh tabs/splits, selected worktree and Machine.
- Saved Desktop Task layouts, then config templates, then builtin layout.
- Files, Changes, Shell and Task/workflow tools. Native hosts own arrangements;
  LF persists no host workspace inventory or mutation history.
- Normal `lf resume <session>` behavior for an explicit conversation selection;
  `lf resume --task <id>` when a Task has no conversation yet.
- Started unfinished Tasks only by default. Automatic arrivals keep native focus
  and existing input ownership: show Task/workflow in the conversation slot until
  explicitly opened. This is the minimal implementation choice needed to avoid
  background `resume` moving somebody's input.
- Actual working-directory reporting/inheritance for native splits and manually
  resumed conversations. No changes to normal resume's screen protocol.

## Removed — do not reintroduce

- `open/workspace/observer.rs`: launch keys, files, locks, readiness waits/retries.
- `lf session view`, its hidden observer flag, ObserverState and readiness DTOs.
- Workspace-specific passive transport, Ctrl+R transfer, transfer placement fields,
  OutcomeUnknown wrapper and the just-added read-only hint.
- Tests exclusive to that path; use native launch + normal resume tests instead.

The prior design, failed demos and superseded constraints are retained in
`21bbb00329:scratch/rex-workspace.md` and `21bbb00329:scratch/workspace-demo.md`.
No shipping or installation requested. PR #1814 remains the delivery.


Jack Heart also requested that `docs/` read as user documentation. The workspace
guide now starts with commands and describes behavior, configuration and recovery.
Design acceptance dates, testing provenance, delivery status and internal Task
ownership remain in scratch/PR evidence instead of the guide.

Jack Heart rejected the November 6 demo showing only Task & workflow. The immediate demo must be one ordinary Task conversation in a Rex tab. A status tool is not a substitute for working agent input; do not present partial layout creation as that experience. This does not remove the previously requested optional Task/workflow tool.

Jack then confirmed “okworked” after the single ordinary Claude conversation was
opened. Use that experience as the target; the branch's fresh-start failure remains
unresolved. This feedback approves neither shipping nor demo cleanup.

Jack Heart requested the PR against LOO-717. LOO-498 now selects LOO-717 / PR #1811 as its stack parent. Keep shell lifetime and retirement in that parent; review native opening and cwd additions in #1814 above it. The failed branch fresh-start demo remains unresolved.

Stack validation against LOO-717 a1067c99: isolated Rex resume/cwd tests 2 passed; last-viewer/disconnect retention test 1 passed; formatting and all-targets clippy passed. No live demo was relaunched during stacking.
