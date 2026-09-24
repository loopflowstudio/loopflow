# Normalized Watch paging review — 2026-09-24

The normalized paging slice passes after one desktop recovery correction.
Refresh the in-progress PR. The complete Watch feature still needs discovery,
initialization and inventory bounds, complete capture, visible polling, exact
checkpoint Session navigation and the configured human demo.

Read the Task directive, active design and complete working delta over
`4fe578a5a894`, including the implementation and compression reports. Compared
the base-to-working-tree inventory with the preceding reader, feed, diagram and
retention reviews. Those inherited reviews are not fresh whole-branch validation.
The complete non-scratch base patch is retained locally at
`/tmp/loo293-page-review-complete.patch`.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Expanded-message pages | At most 128 records / 8 MiB serialized record payload per source | Shared SourcePage admission; Claude resumes at a content block | Source-matched 14-test reader receipt; 259 blocks read as 128/128/3 through CLI and desktop | pass |
| Identity and live arrivals | No missing/repeated blocks; tool correlation crosses pages; independent live tail | Source IDs remain stable; tail clears historical block progress | Existing reader and desktop receipts include separated tool call/result, one appended arrival and quiet continuation | pass, synthetic source |
| Oversized records | Explicit gap, then later output remains reachable | Oversized normalized record is consumed; full page defers its next record | Claude/Codex payload-expansion tests and OpenCode 3+3 paging | pass |
| Pending source replacement | In-place replacement cannot inherit a block index | Digest resets the pending complete line | Existing same-inode pending-line replacement regression | pass |
| Old cursor recovery | CLI rejection offers usable Reload output | Review found missing button on ordinary read errors; button now available with existing reload behavior | Before test fails to find button; after passes both reset/error cases; configured CLI rejects version 2 and desktop recovers 59 rows | pass after correction |
| Passive observation | Watch does not mutate provider or execution state | Existing read-only Task lookup, native readers and RegistryQuery | Configured history read plus reachable-source inspection and negative searches | pass for observed reads |
| Full Watch experience | Automatic arrivals, all supported native capture, stage transitions, checkpoint Iterate and completed reopening | Manual updates; other bounds and navigation remain unfinished | Current design and view/read source | gap, remains in this PR |

## Demonstration and correction

The new cursor version tells older retained presentations to reload. The view
previously showed Reload output only when a successful page reported a source
reset. A failed continuation instead left the user with stale output and no
reload action. The two-case regression reproduced that failure before the fix:
[before](watch-page-evidence/review/reload-before.log).

Expose the existing Reload button for every output read error, disabled during
reads. No error-string parsing, new error DTO, compatibility reader or second
recovery path was added. The existing view callback resets both presentation
continuations through TaskWatchStore. The regression activates the native SwiftUI
button through ViewInspector and checks recovered output and cleared errors for
both source reset and cursor rejection. Both cases pass:
[after](watch-page-evidence/review/reload-after.log).

The [configured probe](review-page-proof.swift) reads actual LOO-293 through the
branch CLI, RegistryQuery and TaskWatchStore. It alters only the returned cursor
version in the viewer's temporary response to simulate a retained pre-upgrade
continuation. The next real CLI read rejects it. Previous output remains intact;
the Reload button recovers all 59 prior rows from three sources; a later Follow
read has unique source-local row identities. One test passes:
[configured receipt](watch-page-evidence/review/configured.log).

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter TaskWatchFeedTests/resetAndReload
swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter WatchPageCLIReview
```

The second command used a temporary copy of the archived probe in the test
target, removed after its pass. Its first compile found an unsupported nested
`#require` in the probe; splitting that expression fixed the probe only. This is
configured historical-read and programmatic-control evidence, not physical UI
interaction, new provider output or the complete human Watch demo. No production
Task, transcript, Session or installed executable was changed to manufacture it.

All eight implementation receipt hashes still match, including Rust readers,
desktop consumers, the paging integration probe and branch CLI. Reused their
14-test Rust and one-test synthetic desktop results without repeating unchanged
proof. The latter establishes the large-message boundary unavailable in the
configured three-source population. New Swift proof compiled and linked the Mac
product. Formatting, full-target Clippy and whitespace checks pass. Final review
[hashes](watch-page-evidence/review/hashes.json) identify executable source and
probes. No full repository or hosted UI gate ran.

## Source review and next step

Traced SourcePage admission through JSONL offset/block/digest continuation and
OpenCode keyset/revision admission, then the Task envelope, cursor-file transport,
desktop merge and reload control. OpenCode commits a seen revision only after
acceptance, so a deferred part remains readable. Claude builds at most one
bounded block batch before yielding; identity and tool folding are unchanged.
The budget measures serialized record payload, not response overhead or RSS.

Negative searches find no Watch provider launch/resume, Session action, direct
Swift provider-file/database reader, WebSocket, new transcript writer or removed
DTO aliases. Native SQLite opens read-only. The `Command::new("claude")` match
in native-source code is a test inspecting an unspawned command's environment.
No plan reconstruction or skill-name attribution was added by this slice.

This advances the accepted architecture. It does not establish polling readiness:
discovery still scans the complete manifest inventory, tail seeds every known
source, and snapshot/source inventories and cursor growth remain separate bounds.
The source-level cap can be multiplied by eight sources per Task page, and
transient decoding is still outside the retained desktop budget. Bound those
paths next without losing discovery or continuation. Keep complete capture,
exact checkpoint Session links and the configured end-to-end human demo in this
Task and PR. No landing or Task completion is authorized by this review.
