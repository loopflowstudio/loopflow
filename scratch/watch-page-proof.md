# Normalized Watch page bounds — 2026-09-24

The existing source reader now limits each page to 128 normalized records and
8 MiB of serialized record payload. Previously the JSONL limits covered source
lines: one Claude message could expand into thousands of output records, each
repeating its message identity. The desktop's retained window acted only after
that entire response had been allocated and decoded.

## Ownership and behavior

SourcePage applies the payload budget to journal, Claude, Codex and OpenCode
records. A full page leaves the next record unread. A single normalized record
larger than the budget reports `record_too_large` and advances; later output
remains reachable. This is explicit missing evidence, not complete capture.

The private JSONL cursor retains a pending complete line's digest and next block
index. Claude resumes inside that line without changing source IDs, revisions,
tool correlation or source order. Normalization processes at most 128 blocks
from the line in one request rather than building all its output records first.
It may reread that bounded line; it does not reread the whole transcript.
Replacing the pending line resets the source even when its inode and preceding
anchor are unchanged. An independent tail clears historical block continuation
and starts after the last complete line. Existing partial trailing-line behavior
is retained.

OpenCode publishes its part revision only after the normalized record fits.
Otherwise its keyset position remains before the unread part. Existing inclusive
timestamp boundaries and unfinished-part tracking remain unchanged. No transcript,
Run manifest, provider Session or Task flow fact is written by the reader.

The opaque Task output cursor version is now 3. Retained older continuations
request Reload output; there is no compatibility reader or durable migration.
Public output DTOs, fixtures, CLI commands and Swift production code are unchanged.

## Proof

`cargo test -p loopflow --lib run_record::output::tests:: -- --test-threads=1`
passes all 14 selected tests. [Reader receipt](watch-page-evidence/reader-tests.log).
The three new regressions prove:

- A 259-block Claude message pages without lost or repeated IDs, preserves tool
  call/result identity across the page boundary, and retains independent live
  arrivals. Rewriting its pending line in place resets to the new first block.
- Claude and Codex identity expansion respects the serialized payload budget;
  an individually oversized record produces a gap while all other records,
  including subsequent output, remain readable exactly once.
- OpenCode identity expansion pages six records as three and three without
  committing an unread revision or replaying records on a quiet continuation.

Built the branch `lf` with `cargo build -p loopflow --bin lf`. The archived
[desktop integration probe](watch-page-proof.swift) was temporarily copied to
the Swift test target, run with `swift test --package-path swift -Xswiftc -gnone
--jobs 4 --filter WatchPageCLIProof`, and removed after its one passing test.
[Desktop receipt](watch-page-evidence/desktop.log). SwiftPM compiled the Mac
product and test target. No screenshot or physical UI interaction is claimed.

The probe creates disposable Run/native-history files attributed to LOO-293 and
uses the real registry only through the existing read-only Task lookup. Its
RegistryQuery invokes the freshly built branch CLI. The actual TaskWatchStore
loads 128, 128, then 3 historical records, folds the separated tool call/result,
retains Run filtering, clears it through Follow live, accepts one appended
arrival, and remains at 260 records after a quiet read. Its source and temporary
files are removed on completion. This is synthetic provider-source integration,
not configured provider capture or the human Watch demo.

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
`git diff --check` pass. [Clippy receipt](watch-page-evidence/clippy.log).
[Hashes](watch-page-evidence/hashes.json) identify the tested reader, binary,
desktop consumers and probe. No executable edits followed the passing proofs.
No affected-suite or full repository gate ran.

## Review and remaining work

Review traced continuation through the existing source reader, Task envelope,
opaque cursor transport and desktop merge. Two necessary consequences are
covered: tail startup must clear an initial partial-message cursor, and an
OpenCode revision must not become seen before its output is accepted. The line
digest prevents a replaced pending message from inheriting an obsolete index.
No new DTO, command, cache, transcript authority or Session action was added.

The budget covers serialized record payload only. It does not bound all source
metadata, gaps, manifests, cursor size growth, source/snapshot inventories,
discovery, tail initialization, transient decoding or process RSS. The Task page
can contain up to eight sources. Automatic polling remains off until its other
bounds are implemented. Complete capture, exact checkpoint Session navigation,
the configured end-to-end Watch proof and human demo remain required in this PR.
Nothing was installed, published, landed or marked complete in this slice.
