# Observation-order Watch review — 2026-09-24

## Independent committed-head check

Reviewed `6e4538549` against `782dd3e75`, the complete Task directive read from
the configured registry, the current slice and full target, and the earlier
foundation/feed/diagram boundaries. The tree was clean. No executable correction
was needed; the applicable observation-order slice claims pass. Refresh the
in-progress PR, without landing or completing the Task.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Alternating Runs | Contiguous A/B/A blocks, adjacent prose folding | Shared observation positions and derived groups | Matching committed-source hashes, 12-test model receipt and inspected feed render | pass |
| Late history/revisions | One current value, history cannot overwrite live; exact changed-row focus | Per-record position, live precedence and last-change ordinal | Existing overlap, tool-correlation and exact-Follow tests; source trace | pass |
| Desktop path | Existing reads supply diagram/feed and preserve auxiliary Runs | RegistryQuery → retained store → native views | Matching two-test real CLI-to-desktop receipt; inspected integrated render | pass locally |
| Passive configured read | No provider control or manufactured history | Three Runs; three readable quiet output sources; absent plan explicit | Fresh read from `/tmp`, receipt below | pass for reading; no live-arrival claim |
| Full Watch | Automatic arrival, complete capture, checkpoint navigation and configured demo | Still incomplete | Accepted directive and current implementation | gap, retained Task scope |

All six production/test hashes, the branch reader binary, and both review-probe
hashes match `watch-observation-evidence/review-hashes.json`. Reused the exact
source-matched model/render and two-test CLI-to-desktop receipts rather than
rerunning unchanged suites. Inspected both `feed.png` and `workspace.png`:
alternating provider/Run labels, source warning counts, tools/prose, connected
stages and explicit manual-update status remain visible. These renders do not
prove physical scrolling or the installed-app demonstration.

Fresh `task watch LOO-293 --json` returned three Runs, zero invocations and
`position_unavailable` in 0.562 s. Fresh `task output LOO-293 --json --tail`
returned three available sources, zero records and no gaps in 2.029 s. Both used
the branch CLI from `/tmp` against the existing read-only registry with empty
stderr. No provider or Task was mutated. Counts-only receipt:
`/tmp/loo293-independent-observation-read.json`. Quiet output and missing plans
cannot establish the full configured demonstration.

Independent negative searches confirm the Watch path calls only the existing
snapshot/output reads. Removed order arrays, latest-Run target and legacy DTO
names remain absent. No direct Swift provider reader, durable transcript,
parallel Session inventory, lifecycle action or additional service API appeared.
The source/reference/position types describe transient display state. Keep the
next discovery/retention work at these existing owners; no speculative API is
needed by this review.

Before PR refresh, formatting, full-target Clippy and whitespace checks passed.
Only this report changed during the independent check; no new behavioral suite
or configured live-arrival verdict is implied by those static checks.

## Earlier review evidence

The observation-order slice is coherent for PR refresh. It advances the accepted
one-PR Watch design; it does not establish full Task acceptance. No bounded
production defect was found and no executable production/test file was changed
by this review. Automatic polling remains disabled.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Concurrent arrivals | A/B/A stays three labeled blocks | Global presentation positions, derived contiguous groups | New real CLI → RegistryQuery → Store probe; existing `observationBlocks`; inspected complete feed image | pass |
| Prose order | Adjacent deltas fold; another Run breaks folding | Folding requires consecutive observation positions | Existing ordering/interleaved-prose tests, source review | pass |
| Revisions and history | No replay; history cannot replace newer live content | One revision per source record; independent position/live precedence | Existing late-history/revision tests; new quiet continuation check | pass |
| Late tool completion | Update original row and follow that exact row | Call correlation retains command/input; change ordinal selects row anchor | New passive native-file append through Rust reader and desktop; existing native/tool revision cases | pass |
| Filters and Follow | Clear stage/Run filters, preserve auxiliary Runs | Existing retained selection plus composite source/row target | Existing filter proof; actual Follow button action in new CLI probe | pass, control/target only |
| Source evidence | Empty/unavailable sources remain visible | Sources disclosure retains labels, warnings and paging; collapsed warning counts | Inspected full feed render and source | pass |
| Integrated appearance | Diagram, attempts and alternating output share Watch | Existing connected diagram above feed; native scrolling | Inspected `workspace.png` and `feed.png` | pass, fixture render |
| Configured history | Read existing Task evidence without provider control | Three sources, 59 display rows, no retained invocation | New configured CLI-to-desktop read from `/tmp` | pass, historical read |
| Complete live Watch | Visible polling, capture, exact checkpoint links and configured transition/Iterate/reopen demo | Still incomplete | Design, reader/view source, prior evidence | gap; required in this Task |

## Demonstrated path and limits

[Preparation](review-observation-proof.py) creates disposable Claude/Codex-shaped
native files with two Run manifests attributed to the existing Task.
[Probe](review-observation-proof.swift) invokes the branch `target/debug/lf`
through RegistryQuery, using the actual read-only Task registry. It substitutes
binary selection, not DTO responses. It seeds both continuations, loads initial
tool calls, appends Claude/Codex/Claude prose across three reads, then completes
the first tool after the later prose. Assertions prove three live blocks, exact
text order, stable block identities, retained tool input/output, the original
row becoming Follow's target, quiet deduplication and the rendered Follow action.

The second probe reads the configured `/Users/jack/.lf` Home without modifying
its Task, Runs, provider files or clients. Three sources produce 59 rows; zero
invocations remain truthful missing-plan evidence. No plan was fabricated for
that observation. Synthetic appends do not prove actual provider persistence,
client survival or the configured human demonstration.

Command: `swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter
WatchObservationCLIReview`. Both tests passed, including Mac compilation/linking.
The scratch probe was copied temporarily into the test target and removed after
its pass. [Receipt](watch-observation-evidence/review-cli-desktop.log),
[binary/source hashes](watch-observation-evidence/review-hashes.json),
[render](watch-observation-evidence/review-native-output.png). The render shows
completed historical command/input/result and separately labeled arrivals;
remaining blocks are scrollable. It is a window-backed render, not physical
scroll input or proof that ScrollViewProxy visibly reached its target.

Reused the existing twelve-test model receipt and final three-test receipt
(including two render cases). All six source/test hashes still match
`source-hashes.json`; no executable edits followed those passes. Inspected both
complete-feed and integrated-workspace PNGs. No broad suite or unchanged Rust
checks were rerun solely for this review phase.

## Source and architectural review

Read the accepted directive and active design, reviewed the full current working
delta, and checked the base-to-worktree file inventory against the earlier
foundation, feed and diagram reviews. Those earlier reviews retain responsibility
for their unchanged historical slices; this is not fresh validation of every
inherited LOO-291 change.

Traced the demonstrated reader through native normalization and TaskOutputPage,
RegistryQuery transport, retained TaskWatchStore, record merge/row folding,
derived groups and ScrollViewReader anchors. Navigation still owns each Task's
presentation; the mounted view owns cancellable reads. FlowPosition/transactional
Task history and native/journal files remain the authorities.

Negative searches of reachable Watch models/views found only `taskWatch` and
`taskOutput` reads: no Session open/complete/resume/interrupt, direct database or
network reader, timer or polling loop, or durable transcript writer. Removed
order arrays, membership sets, `containsRevision`, `latestOutputSource`,
`TaskOutputRecord` and `TaskWatchStepKind` are absent from Rust/Swift sources.
No new command, DTO, fallback parser, Session inventory or flow reducer appears.

Position, live precedence and last change remain separate facts: receiving an
identical live revision can establish precedence without creating an arrival;
a tool completion changes its earlier row without moving it. Composite row
anchors prevent identical provider item IDs in different Runs from colliding.
Source evidence is shown once instead of repeated on every contiguous block.
This keeps the next retention work within the existing owners.

## Next slice and disposition

Bound discovery, tail initialization, cursor growth and retained display records
before adding the visible one-second polling loop. Preserve independent history
and live cursors, exact revision precedence and current source/reset evidence.
Then finish complete autonomous/native capture and configured provider proofs,
link checkpoints through exact shared Session identity, and demonstrate stage
advance, inspection/Follow, concurrent Run, Iterate and completed reopening in
the configured desktop before the pinned human gate.

Approve this slice for PR refresh under review-slice. Full Watch acceptance,
physical scrolling, complete provider coverage and the configured human demo
remain open. No landing or Task completion is authorized by this review.
