# Direct Task opening review — October 6, 2026

Design: [LOO-371](open-tasks-directly-and-optimize.md).
Prior comparison: [October 5 measurements](../scripts/benchmarks/desktop-performance/20261005-task-open/README.md).

Jack Heart requested autonomous demonstration and measurement, without operating
live copied providers. During this review Jack asked how LOO-371 compares with
LOO-382 and whether the approaches are compatible or redundant. Jack then wrote “ok, approved. complete,” approving this review and requesting
Session completion. This does not supply missing visual or combined-build proof.

## Relationship to LOO-382

The agent inspected `lf roadmap --task LOO-382 --json`, its checkout design and
both versions of `PodiumModel.swift`. LOO-382's published change is
[PR #1452](https://github.com/loopflowstudio/loopflow/pull/1452).

371 owns direct navigation, destination-window reuse, and avoiding redundant
reads when the requested Task/Session is already loaded. 382 owns keeping those
readings current through its workspace stream, replacing refresh timers, and
reducing planning-query cost. Its checkout still uses the older read-first
deep-link path. Neither implementation subsumes the other.

Integration recommendation, not a new decision from Jack: retain 371's routing
with 382 as refresh owner. Both touch PodiumModel, PodiumView and the measurement
harness. Do not introduce another cache or refresh loop. Verify authoritative
move/deletion frames supersede older deep-link evidence, and that opening after
a cross-process change preserves Session identity, draft, layout and scope.
371's `loadedTask` currently considers both roadmap and an earlier task-link
reading; that fallback deserves explicit move/deletion coverage. This is an
integration risk identified by inspection, not a reproduced defect.

382 records warm planning reads around 190–200 ms and cold reads at 1.1–2.3 s
on its own snapshot. Those are reader timings, not comparable rendered Task
opening results. They may reduce 371's cold bottleneck; combined measurement
must establish that. Reuse the existing snapshot and native runners.

## Demonstration and remaining proof

The October 5 comparison remains bounded evidence: five attempts per scenario,
one window and no resolving sheet on the branch; warm median 152 ms and reopen
38 ms passed 250 ms, cold 8,360 ms missed 5,000 ms. Host load and bitmap/OCR
limits remain. Session input, reliable focus and real application deep-link
launch were not established. No p95 or sustained-use KR follows.

A fresh three-sample-per-scenario run uses the preserved October 5 snapshot,
current branch and installed lf, with reports under
`/tmp/loo371-demo-20261006`. Only the private Home copy is used for reads;
copied provider clients cannot connect. This is a current-branch demonstration,
not a fresh baseline comparison. All nine attempts passed their existing budgets:

| Scenario | Samples | Median / max ms | Budget ms |
|---|---:|---:|---:|
| Cold workspace | 3 | 1,668 / 1,710 | 5,000 |
| Warm Task | 3 | 160 / 163 | 250 |
| Reopen Task | 3 | 56 / 56 | 250 |

[Fresh reports and transition journal](../scripts/benchmarks/desktop-performance/20261006-task-open-demo/report.md)
retain the evidence. One window, no sheet, no timeouts; key window remained -1.
The installed lf hash changed to `3c59cb7d…`; load was 25 before, 18 after.
The cold improvement cannot be attributed to this branch or LOO-382 from this
run. Cold workspace construction passed here; real app cold-launch deep links
remain unmeasured. No p95 with three samples. Native output logged AttributeGraph
cycles despite the passing endpoint; their user impact is unestablished.

Next useful action: combine routing with the stream and demonstrate Task open
→ cross-process rename/move/completion → reopen, plus a retained Session input
check. Extend LOO-376's release-bundle runner for the cold deep link before
claiming its budget. Keep the existing numeric budgets and record failures.
Jack approved after the compatibility discussion; no additional design change
was requested. This review supplies feedback, not a Flow navigation verdict.
