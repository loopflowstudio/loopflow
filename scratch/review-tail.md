# Independent continuation review — 2026-09-24

Disposition: pass the current reader slice at local proof level. Refresh the
in-progress PR, without landing or completing LOO-293. No production correction
was needed. The full desktop Watch experience remains incomplete.

Read the accepted Task directive from the configured registry without mutation,
the current design and complete target, the prior inspector review, and the full
working continuation diff above `5fcf327db`. This review does not re-approve the
inherited main-view-task work or upgrade its configured demonstration evidence.

The human clarified the product direction during review: desktop views are the
deliverable, and API growth should be minimal. The existing snapshot and output
reads are sufficient starting points for the feed. Source search finds no
desktop caller of `taskOutput`; the current view only consumes `taskWatch`.
The next implementation must connect output to that view and address bounds
needed by visible polling, rather than pursue more standalone CLI scope.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Independent continuation | Live arrival does not wait for unread history | Tail seeds each discovered source; both cursors use the same reader | Fresh disposable-source CLI proof below | pass, local |
| New auxiliary Runs | Preserve their first output | Runs discovered after initialization read from the beginning | Fresh CLI proof and existing 51-Run test | pass, local |
| Complete-line/session/reset handling | Retain partial lines and reject wrong Sessions; expose resets | Tail seeks only past complete lines and retains existing identity/anchor checks | Existing native-tail test plus source inspection | pass, local |
| Journal capture mode | Avoid summary/canonical replay after a seek | Tail reduces attempt mode from its bounded window; uncertain mode reports a gap and reads history | Existing journal-tail test, configured receipt and source | pass within recorded capture contract |
| Mutable parts | Preserve older unfinished and inclusive-boundary revisions | Read-only OpenCode snapshot retains unfinished IDs and current boundary | Existing OpenCode-tail test plus fresh CLI unchanged-timestamp completion | pass, local |
| Existing transport | Reuse the output DTO and private cursor-file path | Both CLI dispatch paths pass the option; RegistryQuery adds it to the existing read | Source, current CLI build, existing Swift fixture/transport receipts | pass; no desktop feed caller yet |
| Passive authority | Observation cannot control providers or author history | Existing receipt resolution and read-only readers remain owners | Reachable source search and review | pass, source |
| Desktop outcome | Labeled feed, filters, Follow live, connected stages, exact Session links | Plan/attempt inspector exists; feed remains unwired | Desktop source search and prior inspector review | gap, required in this PR |
| Bounded/full configured coverage | Sustainable polling and complete native/autonomous output | Discovery/initial seeding/state remain inventory-dependent; summary-only capture remains limited | Design limits, configured quiet receipt and source | gap, required in this PR |

## Fresh proof and prior receipts

Built the current CLI with `cargo build -p loopflow --bin lf`, then ran
`uv run python scratch/review-tail-proof.py`. The script uses the actual CLI
from `/tmp`, the configured Task registry read-only, and disposable manifests,
Claude/Codex-shaped JSONL, journal records and a WAL SQLite OpenCode fixture.
It starts no provider processes and writes no production Task/provider data.

Result: five arrivals from five sources while four original sources still had
unread historical pages; zero source gaps and zero replays on the next live
read. This includes a newly discovered auxiliary Run and completion of an older
OpenCode part without a timestamp change. It proves the CLI/reader boundary,
not real provider live behavior or a desktop interaction. The first script run
used an incorrectly nested synthetic journal envelope and correctly received
`journal_schema`; correcting the fixture to the existing flattened envelope
made the proof pass. No product code was changed to accommodate it.

Fresh `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`
and `git diff --check` passed. No broad suite was rerun for this report-only
review; the executable continuation changes retain their focused receipts.

Inspected and reused the implementation receipts: three tail-reader tests in
`/tmp/loo293-tail-readers-final.log`, the 51-existing-Run/new-auxiliary test in
`/tmp/loo293-tail-discovery-final.log`, and two Swift fixture/transport tests in
`/tmp/loo293-tail-swift.log`. Existing configured `watch-tail-proof.json` observed
three available but quiet sources: it does not prove fresh live arrival.
Prior inspector rendering/navigation receipts retain their original scope.

## Ownership and remaining acceptance

No new command, DTO, cursor schema, persisted transcript, Session inventory,
launch route, or execution authority is introduced by this slice. `--tail` only
chooses an initial reader position. Source resolution is shared with ordinary
history. Cursor contents remain private reader state transported by the existing
temporary-file path; native writes/launches and account selection are absent
from that path. Swift still owns inspection state rather than provider storage.

Initial source discovery/seeding and retained per-Run/unfinished-part state are
not bounded independently of inventory. Arbitrary backdated/equal-count OpenCode
rewrites, complete summary-only tool capture and faithful provider attribution
across attempts remain unresolved. Keep these gaps visible while integrating
the feed. History/live merge must not let an older historical revision overwrite
newer observed output. The full configured desktop demonstration and pinned
human gate remain mandatory; this reader pass does not satisfy them.
