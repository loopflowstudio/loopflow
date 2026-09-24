# Reader/snapshot recheck — 2026-09-23

Disposition: the reader-repair/snapshot slice at `63bfcb9b7` passes this recheck.
The active checkout subsequently advanced to a new Mac inspection slice during
review. That work is unreviewed here, so no combined-tree publication is made.
This report supplements the prior `review-slice.md`; it does not overwrite its
dated correction or approve the complete Task.

## Scope and evidence

Reviewed the accepted Task directive and the reader/snapshot `This slice` present
at turn start, the branch changes from `88cf10641`, and prior provenance, capture,
fixture, and recovery receipts. The preceding compression pass already compared
the complete Watch/output mirrors. This pass traced execution writers, passive
read dispatch, native normalization, and discovery behind fresh CLI proof.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Retained flow | Exact plans, bindings, retries, Iterate and settlement | Transactional Task facts, one Rust projection | Writer/reducer inspection; prior focused store and fixture passes | pass |
| Missing evidence | No invented old plan or completion | Missing plan/Run gaps; unavailable position storage stays unknown | Fresh configured Watch returns 3 Runs, 0 invocations, `position_unavailable` | pass |
| Native correlation | Concurrent calls pair with reversed results | Native item ID shared by call/result; distinct source records | Fresh synthetic CLI: 4 records per Claude/Codex source with preserved IDs | pass |
| Partial discovery | Healthy evidence survives unrelated damage | Manifest/directory errors become explicit gaps | Fresh CLI retains 8 output records and 2 Watch Runs beside corrupt manifest and unreadable prefix | pass |
| Shared readers/recovery | Other Run readers survive; repaired storage clears uncertainty | Shared scanner continues through subtree failures | Fresh `lf runs` returns 2 synthetic Runs; recovery returns 8 output records with no discovery gaps | pass |
| Passive configured output | Read retained provider history without takeover | Direct read-only dispatch and exact source resolution | Fresh output: 197 records, 3 sources, no source/discovery gaps | pass for historical read |
| Complete Watch | Live feed, bounded history/discovery, filtering, Follow live, native capture and configured demo | Incomplete at reviewed revision; Mac inspection work now underway separately | Source and active design | gap, subsequent work |

Built the branch CLI with `env -u LF_RUN_ID cargo build -p loopflow --bin lf`
(24.78 seconds). Ran `scratch/review-output-proof.py` with `LF_RUN_ID` unset,
`LF_CONTROL_HOME=/Users/jack/.lf`, and
`LF_CONTROL_DB_PATH=/Users/jack/.lf/loopflow.db`. The script writes a disposable
synthetic Run tree, reads the configured Task registry, and enforces its directory
permission precondition. All assertions passed, including error recovery.

Separate configured reads used that rebuilt binary from `/tmp` and the same
explicit Home/database. Watch matched the exact Task ID in 2.294 seconds; output
matched it in 0.373 seconds. Both had zero stderr bytes. History remained pending;
these reads establish neither fresh arrival nor a live diagram. No provider
client or production Task/transcript was mutated. `uv` emitted its environment
path warning outside the CLI; neither invocation failed.

## Architecture and limits

All four position-deletion paths retain settlement in the owning transaction.
Worker bindings follow exact claim checks; human transitions retain the actual
target. Flow facts are excluded at both observation-outbox and shared-store
notification boundaries. Replacement never marks a bound attempt completed.

Both inspection commands bypass runtime setup and share a read-only Task lookup.
Native resolution reads recorded source/account identity without account selection
or credential refresh. JSONL readers open files for reading; OpenCode uses
read-only SQLite. Source-provenance writes remain launch/callback-owned. Swift
uses typed CLI projections, with no legacy decoder or provider-storage reader.
No second lifecycle, transcript store, migration, or observation-side provider
launch is introduced by the reviewed slice.

Unchanged implementation tests were not rerun solely because review began.
The earlier focused results remain prior receipts; this turn contributes the
fresh build and CLI proofs above. `git diff --check` passed before the new UI
work began. No new broad gate, lint pass, or configured UI demo is claimed.

## Moving checkout and next review

During this pass another run replaced `This slice` with Mac snapshot inspection
and began adding `TaskWatchStore.swift` and `TaskWatchView.swift`. Those changes
and further edits are preserved. The existing publication command stages the
working tree, so refreshing now would publish more than the reviewed slice.

The invoked review-slice skill requires publication when “all applicable
`Done when` claims hold and the slice is coherent.” That is established for the
reader/snapshot boundary, not the newly active UI slice. Finish and review its
focused selection/stale-state/access tests, Mac build, real-view rendering, and
entry-point wiring before refreshing the combined PR. Keep the full live feed,
bounded discovery/state, independent history/live cursors, complete capture,
Session navigation, and configured human demo requirements in this same Task.
No landing or Task settlement was attempted.
