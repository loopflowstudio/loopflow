# Main integration through v0.12.27

2026-09-30 · LOO-298. The owned sync supplied main
`12013dae4fadc4946b885309ebcc22f1061e6493`. Resolution follows Jack Heart's
recorded merge-only direction. No new sync, rebase, push, installed migration or
configured provider operation was started by the recovery agent.

The existing merge completed as `2eeceaeab`. Its first continuation reported
tracked dirty state because cross-file adaptations were outside the original
conflict paths. `lf commit` checkpointed those as `636ca4e27`; a second
continuation confirmed that no sequencer remained. The waiting caller retains
postcondition verification and publication.

## Reconciled behavior

- Keep the shared Task/taskless Flow driver and SQLite Exec, AgentSession and
  FlowSession owners. Do not restore main's old Task launcher, Run records or
  standalone `position.json` store.
- Retain main's merge-based sync, publication base healing, inherited-scratch
  cleanup, account identity/routing, context budgets and progress-comment markers.
- Capture account selection in the invocation. Task preflight, shared driver,
  review children and later resumes use that selection. Resume flags cannot
  silently replace it; native conversation affinity remains intact.
- A repeated Flow node acknowledges seeded/live direction through its exact
  consumed successful native completion and Session input observations. Failed,
  interrupted, unconsumed and other-node results cannot acknowledge it. Missing
  input evidence replays direction. This translates main's successful-visit
  acknowledgement into the branch's exact-completion contract without scanning
  historical sidecar directories.

Review caught two substantive integration gaps: resume flags could override saved
account intent, and main's progress-comment marker still parsed the removed
`RunId`. Both are repaired. Branch-only sync commands and synthetic account
fixtures now use the current command tree and identity fields.

## Focused proof

`.lf/tmp/sync-resolution/check.py` runs the selected cases with inherited
`LF_*`/`LOOPFLOW_*` removed and a disposable Home/database. All Rust test targets
compiled. `focused.log` records nine passes and one failed fixture: main's Claude
stand-in waited for EOF, while the shared conversation driver keeps stdin open.
The exact fixture `cat` was terminated to release the test; its resulting failure
is retained. The stand-in now reads one input and emits a native completion.
`account-check.py` and `account-repair.log` record that case passing afterward.
No production changes followed the nine passing cases.

The ten behaviors exercised were context budget/source retention, reference-only
historical skill mentions, captured Task definition/account retention, exact steer
acknowledgement, progress exclusion from direction, synthetic comment publication,
existing-child stack sync, watched landing with nested repair, Task conversation
retry/review, and mixed-provider Flow account continuity through retry and review.
Provider and planning services were synthetic. No configured provider or Desktop
acceptance follows. Earlier compile attempts exposed renamed helper/Run types
and new account fixture fields; those were reconciled before behavioral execution.

Website docs were synced and architecture HTML regenerated; the generated HTML
remained unchanged. Formatting, diff checks and the repository-required all-target
Clippy passed (`.lf/tmp/sync-resolution/clippy.log`). The integrated gate,
required hosted CI, installed conversion and configured acceptance remain with
the supervising session; this focused recovery does not claim their completion.
