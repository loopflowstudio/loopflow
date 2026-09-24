# Watch observation order — 2026-09-24

The feed now shows contiguous blocks across Task Runs: A, then B, then A stays
in that order. Adjacent prose still folds within a Run, but cannot cross another
Run's observation. Provider/Run/stage labels repeat at each block. History appears
before arrivals with a separate label; these are reader positions, not provider
timestamps or a claim of cross-provider causality.

Each existing TaskWatchOutput retains one chosen revision and its observation
position. History/live precedence remains explicit. The prior order arrays and
membership sets are gone. TaskWatchStore supplies one monotonic presentation
counter and derives the visible groups. Groups and row references persist no
transcript. Native calls/results still fold across pages and messages. Tool
completion updates its original row; Follow targets that exact changed row,
even when later prose or another Run remains below it. Duplicate/quiet pages
retain the target. Reload clears the loaded records and observation sequence.

Source availability, warnings and paging belong to sources, so the view presents
them once in Sources. Unavailable and warning counts remain visible while its
details are collapsed. Sources with no loaded output remain reachable and have
an explicit empty-page explanation. Stage/Run filtering and Follow use the
existing retained selection; no Session action or provider read path changed.

## Focused proof

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter 'TaskWatchFeedTests|TaskWatchOutputTests'
LF_WATCH_RENDER_PATH=/tmp/loo293-observation.png swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter 'TaskWatchTests/renderSnapshot|TaskWatchFeedTests/observationBlocks|TaskWatchOutputTests/followsUpdatedRun'
```

The first command passed twelve tests, including the two-case native tool
correlation test. [Model receipt](watch-observation-evidence/model-tests.log).
The ordering test was subsequently strengthened to use identical Turn/item IDs
in two Runs. The final command passed three tests, including both standalone and
integrated rendering cases, after that test and the final view edits. The Mac
product compiled and linked. [Final receipt](watch-observation-evidence/view-tests.log).
No executable edits followed the final pass; [hashes](watch-observation-evidence/source-hashes.json)
identify its source. No affected-suite/full gate or Rust check was needed for
this Swift presentation slice.

Inspected the [complete feed](watch-observation-evidence/feed.png) and
[integrated workspace](watch-observation-evidence/workspace.png). The complete
feed exposes the alternating Runs, command input/output, auxiliary prose, exact
stage controls and source warnings. The diagram and feed fit the unified Work
workspace. The initial isolated feed render omitted the parent palette/background;
its transparent capture was corrected in the test host before retaining this
receipt. The production Watch parent already supplies those styles.

## Review and remaining work

Review traced typed RegistryQuery pages through the existing record merge,
row folding, derived groups and native ScrollViewReader anchors. No new source
reader, wire field, persisted store, Session inventory or lifecycle control was
introduced. Historical membership and live revision precedence cannot collapse:
a historical copy must gain its historical position without replacing a newer
live value. Display position and most-recent-change position likewise differ
for mutable parts and tool completions.

The new tests prove A/B/A blocks, adjacent prose, exact late tool-completion
focus targets, same-ID isolation between Runs, history/live overlap, filters and
quiet-page deduplication. Existing cancellation, stale evidence, reset/reload
and tool-correlation proofs still pass. Source detail consolidation prevents
warnings from repeating on every block while retaining visible failure counts.

Sorting/folding still visits the retained in-memory records. This does not solve
bounded retention, discovery or tail initialization; polling stays manual.
Configured scroll/expansion interaction, full provider capture, exact checkpoint
Session navigation, the complete configured Watch trial and human demo remain
required in this Task/PR. No provider, installed app or lifecycle state was
mutated. Nothing was published, landed or marked complete in this slice.
