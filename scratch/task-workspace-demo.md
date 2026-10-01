# Task workspace demo — 2026-09-30

LOO-353 Unit 1, [PR #1369](https://github.com/loopflowstudio/loopflow/pull/1369),
reviewed with Jack Heart from `growth-thoughts` at
`4b15f9591ce9062c16b115b3332c83101ab7e91e`.
Design: [growth-thoughts.md](growth-thoughts.md).
Prior local proof: [kickoff-evidence.md](kickoff-evidence.md).
Ask contract: [ask-evidence.md](ask-evidence.md).

Status: configured demonstration blocked at the app/CLI installation boundary.
No Unit 1 behavior or latency target has passed this demo. Jack has not accepted
the experience or requested a product design change in this review.

## Jack's direction

Jack requested the Desktop app on the main Home and real Tasks, covering checkout
association, files without PR/Project metadata, retained drafts when symlinks make
files read-only, pane collapse/focus/restore, participation, Ask retries and latency.
Jack initially said any Task was fine, then suggested LOO-298 or growth-thoughts.
The demo selected LOO-298 for its placed Task workspace; this conversation remains
in the supplied growth-thoughts checkout.

## Executed observations

- The working tree was clean. PR #1369 is open and its head matches this checkout.
- `lf home id --json` identifies the main Home as
  `home_39860354aaca640c2ccb50bf6ca609d8`; the selected published installation uses
  `/Users/jack/.lf/loopflow.db`.
- `lf task status LOO-353 --json` reports no Task. The subsequent Product reading
  does include LOO-353 in its chapter, with no runtime or checkout. This is a
  planning/runtime distinction, not evidence that Jack's Task was deleted.
- `lf task status LOO-298 --json` resolves
  `task_9756ded2ad49449090003636171df943`, checkout
  `/Users/jack/src/loopflow.data-model-one-table-per`, with an active local PR.
  It cannot supply the no-PR case by itself.
- `uv run python scripts/loopflow-dev.py run` succeeded: Swift built, the branch
  CLI compiled with release Home selection and validation-only migration
  authority, the app was signed and launched at
  `/Users/jack/Applications/Loopflow Dev.app`. Observed PID: 66039. The installed
  app at `/Applications/Loopflow.app` remained running separately.
- The development app's `LoopflowDevControl.json` selects
  `/Users/jack/.lf-machine/install/gates/1/lf`. `controlLfPath` uses that selection
  for queries ahead of the bundled helper. That installed CLI lacks `task files`
  and `ask --key`; its Session JSON has no `workspace` field.
- The bundled branch CLI has `task files`, but its `home id --json` fails:
  `store /Users/jack/.lf/loopflow.db belongs to another installation; use its
  installed lf or this build's branch data directory`.
  Changing only the app's helper pointer therefore cannot establish the requested
  main-Home path.
- `lf install promote --help` describes `--from-build` as promotion into a
  disposable installed Home. Source inspection confirms a new development store
  under `.lf-dev/installed/<installation-id>/loopflow.db`. No promotion was run;
  that path is not the requested main-Home demonstration.
- Opening `loopflow://task/LOO-298` explicitly with Loopflow Dev returned success
  from LaunchServices. This proves delivery of the open request, not successful
  Task rendering. Jack subsequently requested reopening; the same command again
  returned success. The conversational claim "Reopened Loopflow Dev on LOO-298"
  overstated the result: only app opening was established.
- The app's AppleScript screenshot command failed with handler error -1717.
  System Events window inspection failed with assistive-access error -1728.
  No agent-captured screenshot or automated native interaction is claimed.

## Jack's screenshot — 17:40 local time

Jack supplied a screenshot of Loopflow Dev showing **Planning unavailable**,
**Roadmap unavailable**, and **Work unavailable** instead of the Task workspace.
The visible decoding failure is `keyNotFound` for `interactions` at
`waves[6].tasks.items[6].flow.record.graph`. The sidebar still displays
**ORPHAN SESSIONS · 26** and Session rows. Their presentation does not prove those
Sessions lack Task placement; the planning read has failed and the older CLI
does not supply the new workspace association.

This screenshot confirms the user-visible consequence of the previously observed
CLI mismatch. Refreshing or repeating the deep link has not demonstrated recovery.
Jack supplied evidence of the failure, not approval or a request to change the
workspace design. Retained local image:
`.lf/tmp/task-workspace-demo/planning-unavailable.png`.

## Measurement boundary

The target remains p95 below 100 ms across at least 20 retained-surface actions,
with zero CLI/Git/network calls caused by those layout actions. Also retain the
separate hierarchy/workspace signals and before/after idle CPU and process counts
with several Tasks open.

The current `retained_workspace_action` endpoint is the next main callback, not
visible/usable completion. The fixture performance runner uses synthetic DTOs and
cannot establish configured Home/provider behavior. A short PID-scoped recording
of this launch is diagnostic only; no layout sample or multi-Task comparison is
claimed. Raw local receipts are under `.lf/tmp/task-workspace-demo/`.

Executed `record_live.py record --pid 66039 --seconds 30 --no-xctrace` at
14:58:06 local time on macOS 26.0.1 / Apple M4 Max. The report contains zero
retained-workspace, hierarchy or workspace-ready intervals. It records 23
`session list` reads (p50 873.51 ms; p95 949.47 ms), plus background activity,
process, roadmap and Wave reads. Those subprocess durations do not measure layout
latency. RSS was 127.2–127.8 MiB (0.5 MiB start-to-end growth). Separate process
snapshots counted five then one app/descendant processes, with app CPU readings
of 0.7% then 0.0%. These are point samples during polling, not an averaged idle
CPU comparison with several retained Tasks. Hitches/presentation were unmeasured.
Receipt: `.lf/tmp/task-workspace-demo/launch-recording/report.md`.

## Remaining walkthrough

| Experience | Required configured proof | This review |
|---|---|---|
| Checkout grouping | Independent Sessions from root, subdirectory and symlink alias group under one Task; sibling/nested checkouts remain distinct | Not exercised |
| Files without planning/PR | Browse, save/read back and refresh on a retained checkout with no active PR or Project metadata | Not exercised; do not delete live planning to manufacture the case |
| Symlink access change | Preserve a dirty draft when an open file or its parent becomes an internal symlink; disable editing, manual Save and autosave | Not exercised |
| Retained panes | Collapse/expand, focus/restore and Task return preserve shell processes, input, file draft, split ratios and selection | Not exercised |
| Participation | A second Task's Ask/Flow indication appears without focus theft; selecting it reaches the exact boundary | Not exercised |
| Ask retries | Same caller/key joins one question and returns retained feedback; only its caller resumes after Jack completes | Not exercised |
| Remote ownership | Owning Home supplies association without local canonicalization of remote paths | Not exercised |
| Latency/resource target | At least 20 configured actions with visible/usable endpoint and idle resource comparison | Not measured |

## Recommended next action

Resolve the supported branch-app/main-Home execution path before resuming the
walkthrough. The current development launcher promises main-Home continuity but
selects an older CLI protocol; its matching helper cannot read that installation.
The implementation owner must reconcile that contract with installation authority,
without relying on copied Home results as live acceptance or bypassing store
ownership. This is a demo finding and proposed implementation direction, not a
new product decision accepted by Jack.

First proof: the app's actual selected helper reads the main Home, emits
`SessionRecord.workspace` and required Flow `graph.interactions`, lists LOO-298's
files, and supports keyed Ask. The native planning read must decode and show the
Task workspace; helper output alone is insufficient. Do not mask this mismatch
by defaulting the missing interaction graph. Then
resume the real Task walkthrough and measure its actions. Capture Jack's feedback
before calling the review accepted. This independent Session has no Flow
completion/navigation verdict.
