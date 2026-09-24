# Watch retention review — 2026-09-24

The retention slice passes after fixing Restart history's handling of an
unavailable source. Refresh the in-progress PR; full Watch acceptance remains
open. Nothing in this review authorizes landing or Task completion.

Reviewed checkpoint `13fc09ea0` and its complete retention delta over `7b9d5d87d`,
the Task directive, current design and full acceptance, and prior reader,
navigation, feed and diagram review boundaries. The complete base-to-head patch
is retained at `/tmp/loo293-window-review-complete.patch`; inherited work keeps
its earlier evidence, not a new whole-branch validation claim.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Retained record bound | 4,096 records across Runs | Existing revision maps evict least recently observed records | Real CLI pages 4,608 Claude/Codex records; exactly 4,096 remain | pass |
| Retained payload bound | 16 MiB, including invisible evidence; oversized records cannot flush smaller output | Cached payload accounting; source page payload cleared after merge | Source-matched payload/revision test and inspected missing-call render | pass, local |
| Useful history and revisions | New history/revisions survive pressure without becoming false live arrivals | Retention recency separate from display and Follow positions | Existing 15-test implementation receipt; real reader history paging | pass, local |
| Recovery | Restart recovers early history, preserves filters and live continuation | Rewinds historical reader only after successful first page | Native Restart and Follow actions; real readers recover 255 early records and both later arrivals once | pass, production-like |
| Failed restart | Keep last-good records and explain failure | Query, cancellation, source availability and discovery failure checked before replacement | Real unavailable native file regression; focused cancellation and discovery-envelope cases | pass after fix |
| Missing tool context | Late result cannot invent an evicted call | Derived correlation uses only retained records; explicit missing-call label | Existing payload test and inspected window image | pass, local |
| Passive configured history | Read existing Task without provider takeover | Read-only Task lookup and recorded source readers | Fresh CLI → RegistryQuery → Store: three sources, 59 rows, zero invocations | pass for historical reading |
| Full Watch | Automatic arrivals, complete capture, exact Session links and configured demo | Still incomplete | Accepted directive and reachable implementation | gap, required in this PR |

## Defect and correction

The CLI can return exit zero with `available: false` for one source. Restart
previously accepted that envelope, cleared all loaded records, and erased its
omission notice. Temporarily moving a disposable Claude history file reproduced
4,096 → 127 records with no output error; Codex alone remained readable.
[Failing receipt](watch-window-evidence/review/source-before.log).

Restart now preserves the previous window and both continuations when the first
page reports unavailable sources or incomplete discovery. Its error names the
source/reason. Successful retry performs the existing replacement; ordinary
paging and live reads retain their existing partial-source behavior. No new
reader, state owner, DTO or persistence was introduced. The permanent recovery
test now covers both a partial-source envelope and a discovery failure, alongside
thrown errors, cancellation, successful retry, filters and preserved live output.

## Demonstration and limits

[Preparation](review-window-proof.py) creates two disposable Run manifests and
Claude/Codex-shaped JSONL histories. [Probe](review-window-proof.swift) invokes
the existing branch `target/debug/lf` through RegistryQuery against the actual
read-only Task registry, substituting binary selection rather than DTO responses.
It seeds the live continuation, reads all 4,608 historical records, crosses the
window bound, writes two arrivals to the disposable files, exercises Restart,
reproduces unavailable-source failure, restores the file, retries, follows both
arrivals, verifies quiet deduplication and reopens a fresh Store.

The first page contains 255 output records because Codex's metadata header uses
one of its 128 reader slots. The initial probe expected 256; its archived
[initial receipt](watch-window-evidence/review/initial.log) is a failed probe
assumption, not a production defect or passing result.

Final command, with the archived Swift probe temporarily copied into the test
target:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter 'WatchWindowCLIReview|TaskWatchFeedTests/restartHistoryWindow|TaskWatchFeedTests/cancelledReadRetainsEvidence'
```

Four tests pass, including both cancellation cases; Mac compilation/linking
passes. [Final receipt](watch-window-evidence/review/source-after.log).
The temporary test-target file was removed after verification; the reproducible
source remains in scratch. Inspected the [CLI-fed window](watch-window-evidence/review/window.png)
and the implementation's [missing-call window](watch-window-evidence/window.png):
retention notice, Restart control, labels and output are readable.

This is real reader/desktop integration with synthetic provider-shaped storage,
not a live provider, installed-app interaction, physical scrolling or human demo.
The configured read is historical and lacks retained plans. Neither establishes
complete provider capture or stage transitions. No production Task, provider
history, installed executable or client was changed.

All nine implementation hashes matched their recorded 15-test and final render
receipts before the review correction. Those unchanged budget/order/correlation
results are reused; the focused final run validates the two corrected files.
[Final hashes](watch-window-evidence/review/hashes.json) identify the production
source, tests, probe and existing reader binary. The reader hash matches the
earlier observation-order review; it is not claimed as a fresh Rust build.
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and whitespace
checks pass. No broader gate or UI automation run was needed for this correction.

## Architecture and next slice

Followed shared read-only Task lookup and native readers through RegistryQuery,
the navigation-retained Store, record merging/eviction, derived rows and view
actions. Negative searches find no Watch provider launch/resume/claim/decision,
direct Swift provider-file/database reader, WebSocket, second transcript writer,
Session inventory, legacy DTO alias or unbounded evicted-call cache. RegistryQuery's
temporary file transports only the opaque cursor. Rust's native SQLite opens
remain read-only; FlowPosition still owns execution and transactional facts own
the diagram. README matches the limited display window and manual updates.

This advances the full design without creating a competing authority. Next,
bound discovery, tail initialization, cursor and inventory growth, incoming page
decoding and navigation-retained Tasks before enabling visible polling. Keep
complete capture, exact checkpoint Session navigation and the configured human
demo in this Task/PR. The 4,096/16 MiB bound is retained transcript accounting per
Task, not an app RSS budget or a claim that the remaining bounds are solved.
