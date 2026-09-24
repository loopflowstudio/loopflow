# Retained Watch output window — 2026-09-24

The desktop now bounds each Task's loaded transcript to 4,096 records and
16 MiB of accounted payload across Runs. This completes the current retention
slice; it does not establish polling readiness or complete LOO-293.

## Behavior and ownership

TaskWatchOutput still owns one chosen revision per source record. It retains
payload cost and retention recency alongside display position and live-change
position. Those positions differ: an updated historical revision keeps its row
position, gains retention priority, and must not become a live Follow target.
TaskWatchStore removes the least recently observed records across its sources
after accepting a page. Duplicate revisions do not gain priority. Oversized
records are omitted without first evicting all smaller readable output.

Accounting includes record identity, prose, tool inputs/results and invisible
event evidence. Item and JSON-value sizes use their existing encoders; strings
use UTF-8 size. This bounds retained payload, not process RSS or incoming decode
allocations. The record cap also limits the number of record containers. The
latest source page's records are cleared after merging so that it cannot retain
a second copy outside the window. The Swift DTO's records property is mutable
for this purpose; its required fields and JSON contract remain unchanged.

A visible notice distinguishes omitted display context from source failure.
Empty states say output is not loaded rather than claiming the reader saw none.
Surviving records keep observation order and correlation. If a native call has
left the window, its later result explicitly says the earlier call is not loaded.
No unbounded call cache or record tombstones replace the evicted payload.

Restart history reads from the beginning and replaces the loaded records only
after a successful, current response. Failed, cancelled and superseded reads
cannot clear them. It preserves the live continuation, source evidence, filters
and selected plan. Load history continues forward; Follow live rejoins the
unread arrivals. Providers and persisted transcripts are untouched.

## Focused proof

```sh
LF_WATCH_WINDOW_RENDER_PATH=/tmp/loo293-window.png swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter 'TaskWatchWindowTests|TaskWatchFeedTests|TaskWatchOutputTests'
```

Fifteen tests pass, including the two-case native tool correlation and two-case
read/restart cancellation tests. [Receipt](watch-window-evidence/tests.log).
The new behavior crosses the actual budgets with eight Runs and several pages,
three six-MiB tool inputs, growing revisions, and an oversized invisible usage
record. It proves that new historical pages remain inspectable, revised history
survives pressure, Restart history recovers early output, and subsequent Follow
reads an arrival from the preserved live continuation. The native Restart history
button is exercised through ViewInspector.

Review then corrected two empty-state labels. The final
`--filter TaskWatchWindowTests` command passes one test and renders the window.
[Final receipt](watch-window-evidence/render.log). Mac compilation/linking passes.
Inspected the [window image](watch-window-evidence/window.png): the retention
notice, restart button, source labels and missing-call results are readable.
[Hashes](watch-window-evidence/source-hashes.json) identify the final executable
source. No executable edits followed that receipt. Whitespace checks pass.

## Review and remaining work

The review fixed two material cases: historical revision updates needed their
own retention priority, and eviction must not make empty-state text claim that
no output was read. The implementation stays within the existing presentation
owners and typed RegistryQuery path. It adds no reader, durable store, CLI
command, wire field, provider action or lifecycle mutation.

This is fixture and rendered-control evidence. It is not configured physical
scroll/input, provider capture, a measured app-memory budget, or the human demo.
Source and snapshot inventories, the number of navigation-retained Tasks,
incoming page decoding, Rust discovery/tail initialization and cursor state still
need bounds. Polling remains manual. Complete capture, exact checkpoint Session
navigation and the configured end-to-end demonstration remain in this Task/PR.
Nothing was installed, published, landed or marked complete by this slice.
