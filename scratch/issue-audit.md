# Outstanding Loopflow issue audit — 2026-10-16

Jack Heart requested an audit of outstanding issues obviated by subsequent changes. The audit covered 72 outstanding Tasks returned by the repository's seven Wave readers, using issue briefs/history, current main, merged PRs, and existing implementation/tests. It did not rerun acceptance tests. A separate broad repository refresh failed on missing Wave goal files; unavailable refresh evidence was not treated as an empty backlog.

Jack Heart accepted the recommendations and explicitly accepted completion of LOO-323, LOO-373, and LOO-445 despite the remaining acceptance-evidence caveats. The following decisions are authorized; the execution results are recorded below after verification.

## Abandon obsolete briefs

| Task | Decision and surviving work |
| --- | --- |
| [LOO-300](https://linear.app/loopflow/issue/LOO-300) | Already explicitly superseded by LOO-304. Old numerical performance/soak targets were subsequently retired. |
| [LOO-317](https://linear.app/loopflow/issue/LOO-317) | Explicit Workflow review nodes and graph rendering replace the collapsed-Flow proposal (#1439, #1492). Current navigation remains in LOO-489/497. |
| [LOO-321](https://linear.app/loopflow/issue/LOO-321) | Automatic private branch Homes were removed (#1381); migration restrictions and explicit clones (#1549) replace that mechanism. Preserve the no-migration invariant and LOO-480 native clone verification. |
| [LOO-322](https://linear.app/loopflow/issue/LOO-322) | Old worker/Flow recovery bundle and automatic landing tail are superseded by explicit Workflow review boundaries. Failed-step recovery remains in LOO-435; landing repair timeout remains in LOO-470. |
| [LOO-361](https://linear.app/loopflow/issue/LOO-361) | Recorded landing intent and explicit waiting replace the old worker and `pr land -c` lifetime contract. LOO-470 retains the concrete timeout defect. |
| [LOO-385](https://linear.app/loopflow/issue/LOO-385) | Deriving Task state from Workflow contradicts the accepted independence of completion, Workflow position, and process liveness. Explicit completion/reopening and no-PR completion shipped in #1499. |

## Complete delivered outcomes

| Task | Evidence or acceptance |
| --- | --- |
| [LOO-337](https://linear.app/loopflow/issue/LOO-337) | #1353 merged with hosted checks passing, satisfying its explicit completion condition. |
| [LOO-379](https://linear.app/loopflow/issue/LOO-379) | Local-first cancellation/deletion save locally and synchronize separately (#1491, #1503); existing tests cover offline operation, lost replies, and conflicts. |
| [LOO-399](https://linear.app/loopflow/issue/LOO-399) | Terminology change shipped in #1520. No whole-word “driver” references remained in inspected current user-facing docs/skills; historical records were excluded. |
| [LOO-323](https://linear.app/loopflow/issue/LOO-323) | Large-prompt transport shipped (#1498, #1518, #1552). Jack Heart accepted completion; the audit itself did not establish the full provider/platform acceptance matrix. |
| [LOO-373](https://linear.app/loopflow/issue/LOO-373) | Landing/startup repairs shipped (#1422). Jack Heart accepted completion; the audit itself did not establish the specific installed reconciliation outcome. |
| [LOO-445](https://linear.app/loopflow/issue/LOO-445) | Cross-provider resume shipped (#1551; follow-up #1562). Jack Heart accepted completion; the audit did not perform the real native demo. |

## Remaining work

- LOO-333: retain configured end-to-end recursive Wave operation/handoff acceptance.
- LOO-392: retain the default-pane Flow graph rendering check.
- LOO-433: retain installed before/after disk measurement for binary cleanup.
- LOO-388: update obsolete provider-only planning assumptions while preserving its ownership invariant.
- The other 56 issues had insufficient evidence for abandonment. Existing active execution, paused work, and independent acceptance obligations remain intact.

## Execution

Verified after mutation with fresh `lf task status --json` reads:

- Canceled: LOO-300, LOO-317, LOO-321, LOO-322, LOO-361, LOO-385.
- Completed: LOO-323, LOO-337, LOO-373, LOO-379, LOO-399, LOO-445.
- All 12 report current planning writeback, no planning error, and non-stale planning state.
- Abandonment rationales were recorded in Task comments; completion rationales were supplied through `lf task complete --reason`.
- LOO-300's rationale comment remains saved locally but pending Linear delivery. An explicit `lf task sync LOO-300` retry reported “Could not find referenced Issue” and could not confirm the Comment. Its Task status read reports canceled/current; do not treat that as proof the pending comment was published. Other abandonment comment commands reported no pending delivery.

Fresh pre-action reads found all 12 selected Tasks unstarted, with no associated checkout, branch, PR, or recorded Task Flow. No execution was launched. The selected 12 dispositions reduce the audited outstanding set from 72 to 60; this is not a claim about unrelated concurrent additions or completions.
