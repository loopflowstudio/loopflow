# Desktop Watch feed review — 2026-09-24

Disposition: the manual desktop feed slice passes after correcting native tool
folding. Refresh the existing in-progress PR. This advances the accepted Watch
design; it does not authorize landing or establish complete Task acceptance.

Read the accepted Task directive, current slice/full target in
`restore-task-watching-with-live.md`, prior foundation/continuation/UI reviews,
and the working feed delta above `2056f4a62`. Inspected the base-to-working-tree
change inventory and retained the full patch at
`/tmp/loo293-feed-review-complete.patch`, including the new untracked sources.
Inherited LOO-291 and reader evidence retains its earlier scope; this review
adds desktop feed evidence and does not reclassify those receipts as fresh tests.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Independent reads | History cannot delay arrivals | Separate seeded live/history continuations through the existing query | `independentHistoryAndLive`; real CLI arrivals probe | pass, local |
| Overlap/revisions | Preserve newer live content without replay | Run/source/item merge, live precedence | Existing focused revision test; quiet CLI continuation | pass, local |
| Native tool display | Keep command/input and correlated result/status | Shared call-ID folding for Claude/Codex native records | Both regression cases fail before fix, pass after; real Rust normalization → Swift view | pass, local |
| Filters/Follow | Exact stage attempts and Run filter; rejoin current progress | Selection stays separate from filters/follow; latest changed Run is scroll target | `filtersAndFollowLive`, `followsUpdatedRun`, CLI probe's Run filter and Follow action | pass, local |
| Failure/reset | Last-good evidence and explicit reload of both readers | Fenced responses, retained errors/gaps, reset freeze | Failed/superseded/cancelled/reset/tail-gap tests | pass, local |
| Native layout | Plan and feed remain visible together | Existing workspace plus split plan/output view | Both window-backed render cases; inspected integrated PNG and CLI-fed view | pass, rendered proof |
| Configured passive history | Read without taking over a client | Existing branch CLI reads the configured Task ledger/output | Real CLI → RegistryQuery → Store: three sources, 59 rows, zero invocations | pass, historical read |
| Complete Watch | Automatic bounded reads, full capture, connected stages, exact Session links, configured demo | Still incomplete | Design and reachable source | gap, required in this PR |

## Defect corrected

The reader intentionally emits native call and result records separately, with
one correlation ID. Codex uses the same turn for both: replacing the display row
with the result discarded the command/name/input. Claude's result is a separate
message: the original call remained labeled running beside an anonymous result.
The original completed-tool test covered a full snapshot and missed both shapes.

The new regression covers two concurrent calls, reversed result order, a failed
result, overlapping history/live reads, and both providers. The initial run fails
with those exact lost-input/stale-status outcomes. Row folding now remembers call
metadata while deriving rows, matches the existing native call ID, and updates
the original row with result/status. It introduces no durable state, DTO, reader,
provider action or new identity. Ordinary journal/OpenCode snapshots keep their
existing folding. Missing earlier calls remain visible as standalone results.

[Before regression](review-feed-evidence/native-before.log),
[final focused run](review-feed-evidence/tests.log).

## Fresh proof and its limits

The final focused command passed 15 tests in three suites, including two native
provider cases and both render cases; the Mac product compiled and linked:

```sh
LF_WATCH_RENDER_PATH=/tmp/loo293-feed-review.png swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter 'TaskWatchTests|TaskWatchFeedTests|TaskWatchOutputTests'
```

Inspected `/tmp/loo293-feed-review.workspace.png`: readable plan/attempts and
labeled output share the unified workspace; history/Follow controls and explicit
manual-update text are visible. No fresh configured scrolling or keyboard verdict
is claimed. The unchanged prior 14-test receipt remains historical.

The installed CLI rejects `task output`. Without replacing that installation,
the closest production-like proof uses the branch reader through RegistryQuery,
the actual retained Store, native NSWindow view rendering, and the real Follow
button action. The query closure selects the branch binary and explicitly permits
only `task watch`/`task output`, whose dispatch opens the Task ledger read-only.
It substitutes binary selection, not JSON or normalized records.

[Preparation](review-feed-proof.py) creates disposable Claude/Codex-shaped native
files and manifests. [Probe](review-feed-proof.swift) seeds live, loads calls,
appends two results, reads arrivals, checks retained commands/completed state,
continues quietly without replay, narrows a Run, and activates Follow. It renders
[the resulting view](review-feed-evidence/native-output.png). A second probe reads
the existing configured Home from `/tmp`: three sources yield 59 display rows;
zero retained invocations remain honest missing-plan evidence. It starts no
provider and writes no production Task or transcript. Synthetic appends prove the
reader/display boundary, not actual provider persistence or live client survival.

The Swift probe was temporarily copied to the test target, run with
`--filter WatchCLIDesktopReview`, then removed from that target. Both tests pass
in [the receipt](review-feed-evidence/cli-desktop.log). Its archived scratch copy
is a local review tool requiring this configured Task; it is not a portable CI
fixture. [Binary/source hashes](review-feed-evidence/hashes.json) identify the
actual proof. The CLI is the existing branch reader build; no Rust changed here.
The ordinary 15-test suite remains the reusable regression proof.

## Architectural review and remaining work

Traced view → retained navigation Store → RegistryQuery cursor-file transport →
read-only Task lookup → attributed source readers, and compared source event
shapes against the display fold. Watch has no direct Swift provider-file/SQLite
reader, Session inventory, provider launch/resume/replace call, polling timer,
flow reducer, transcript writer, or legacy/new adapter. Hidden views cancel their
query task; generation IDs reject late responses. Snapshot/output failures retain
separate evidence. Execution still belongs to FlowPosition; immutable committed
Task receipts supply stages/attempts, and provider history/journals supply output.
No wire fields changed, so existing Rust/Swift fixtures remain applicable.

The next slice must bound discovery/initialization and retained state before
visible polling, then preserve contiguous cross-Run observation order as the
feed grows. Whole-Run grouping is explicitly labeled in this slice. Complete
capture (including limited autonomous journals), configured provider coverage,
connected actual-stage/Iterate diagrams, exact human Session navigation, and the
full configured/human demo remain mandatory in this same Task/PR. A tool-folding
fix and readable history are not substitutes for those outcomes.

No broad suite or Xcode gate was rerun for this Swift-only correction; the earlier
resource-preflight failure remains a separate validation gap. Whitespace checks
pass. No unrelated worker, provider, installed app, Task lifecycle or Project KR
was mutated by this review.
