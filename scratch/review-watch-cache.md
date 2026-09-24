# Recent Watch retention review — 2026-09-24

The four-Task navigation retention slice passes. No production correction was
needed. Refresh the in-progress PR; the complete Watch design still requires
reader bounds, capture, polling, checkpoint navigation and the configured human
demo before its final lifecycle can settle it.

Reviewed the Task directive, current design and complete working delta over
`3a00c7826`, including the implementation and compression notes. Checked the
base-to-working-tree change inventory against the prior reader, diagram, feed
and retention reviews. Their inherited receipts are not fresh whole-branch
validation. The non-scratch base patch is available locally at
`/tmp/loo293-cache-review-complete.patch`.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Four recent presentations | Release least recently viewed store when visiting a fifth Task | One ordered collection; revisits promote the same instance | Four populated fixture stores, weak-reference release after promotion and fifth visit | pass |
| Useful recent state | Preserve filters and loaded output while retained | Existing TaskWatchStore remains intact | Source-matched two-test implementation receipt checks stage, Run and output | pass, local |
| Recover an evicted Task | Fresh snapshot and independent history/live readers; no discarded cursor reuse | New store follows current progress with clear filters | Fixture recovers plan and early output; real configured CLI recovers all 59 prior rows from three sources | pass |
| Follow after recovery | No duplicate rows | Existing revision merge and fresh live continuation | Configured follow checks unique source-local row identities; fixture retains exactly two rows | pass |
| Repository and terminal isolation | Cache stays in navigation; terminals retain their owner | Per-repository navigation in the window's PodiumModel; terminal registry separate | Existing view proof checks repository return, layout retention and hidden Watch removal; owner inspection | pass, local |
| Passive reading | Eviction/recovery cannot control provider or durable history | Shared read-only Task operations and recorded native sources | Configured CLI → RegistryQuery → Store; reachable-source inspection | pass for historical recovery |
| Full Watch experience | Live arrivals, transitions, all capture, exact Session links and human demo | Still incomplete; updates remain manual | Accepted full Done When and current reader/view source | gap, remains in this PR |

## Demonstration

[Archived probe](review-cache-proof.swift) was temporarily copied into the Swift
test target and run with:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter WatchCacheCLIReview
```

It reads actual LOO-293 through the branch's existing `target/debug/lf`, with
explicit control Home `/Users/jack/.lf` and read-only registry selection. It
loads history, chooses a Run, promotes the store, visits four local presentation
keys, and observes that the original store is released. Those extra keys do not
create or query production Tasks. Reopening the real Task starts empty, recovers
every previously loaded row, and follows without duplicate row identities.

One test passes: three sources, 59 recovered rows, zero retained invocations.
[Configured receipt](watch-cache-evidence/configured.log). Missing plans remain
missing; this proves historical recovery rather than live provider arrivals,
configured app clicks, measured RSS or the required human demo. No provider,
production history, Task state or installed executable was changed. The binary
hash matches prior reader evidence; this is not a fresh Rust binary build.

The probe initially failed compilation because it concatenated a structured row
identity as a String. String interpolation corrected the probe only. Its final
source is archived, and the temporary test-target file has been removed.

The two existing focused navigation tests were not rerun: implementation and
test hashes still match their passing receipt. The [copied receipt](watch-cache-evidence/implementation.log)
covers populated-store eviction, filters, fresh plan/history recovery and the
mounted view boundary; [hashes](watch-cache-evidence/hashes.json) identify source
and probe. Swift compilation/linking passes for the additional configured proof.
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
`git diff --check` pass. No broader test gate was run.

## Source review and next step

Traced PodiumModel → WorkspaceNavigation → conditional SessionsView destination
→ TaskWatchView's cancellable task → TaskWatchStore → RegistryQuery → read-only
Task lookup and output readers. There is one navigation collection, no eviction
tombstones or retained discarded cursors. The sheet-local Watch store has a
different view lifetime and uses the same presentation model. Terminal surfaces
remain outside this cache because their release ends their PTYs.

Negative searches find no Watch provider launch/resume, Session action, direct
Swift provider-file/database reader, WebSocket, second transcript writer, or
removed DTO aliases. The native-source `Command::new("claude")` match is a test
that examines launch environment without spawning. Native SQLite reads use
read-only flags. No execution or Session authority moved into navigation.

This advances the complete design without changing its ownership model. Four
stores per repository/window plus each store's payload budget do not bound
incoming decoding, inventories, reader discovery, tail initialization or cursor
growth. Bound those next before enabling visible polling. Keep complete capture,
exact checkpoint Session navigation and configured interaction/demo requirements
in this same Task and PR. No landing or Task completion is requested.
