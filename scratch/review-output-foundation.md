# Output foundation review — 2026-09-23

Follow-up implementation: both reproduced reader findings are repaired. Native
call/result item IDs now retain their correlation identity; unreadable manifests
produce explicit discovery gaps while healthy output remains available. The
rebuilt CLI reproducer passes with reversed results for both providers and eight
healthy records despite the corrupt unrelated manifest. This preserves the dated
review below; current implementation receipts belong in the design's slice ledger.

Disposition: return the combined output foundation to implementation. The narrow
provenance checkpoint remains passed; that verdict does not approve native
capture, continuation, or the complete Watch experience.

## Scope and moving checkout

Reviewed the complete code diff `88cf10641..fb04290e5`, the accepted LOO-293
directive from the existing local registry (read-only), and the supplied design
and evidence ledger. The checkout advanced to `cc2dff810` and `06ba29d57` during
review. The design now calls for the Watch snapshot, and another contribution
has begun adding its read plumbing. Those changes are outside this assessment.
Do not apply this verdict to their final snapshot implementation.

The previous isolated checkpoint report remains in `review-slice.md`, unchanged
by this review. At launch the current slice still explicitly preserved later
reader/CLI/Swift implementation without extending it. This report adds findings
for that preserved implementation; it does not take over the new snapshot work.
No production source was edited or committed by this review. A precautionary
`lf commit` returned “Nothing to commit”; the concurrent commits are not ours.

## Findings

1. **P2 — Native normalization loses tool-call relationships.** In
   `run_record/output/native.rs:123` and `:186`, Claude `tool_use.id` and Codex
   `call_id` are discarded. Normalized call items receive UUID/block or byte
   identities, while results retain only references to the discarded native call
   IDs. Two concurrent calls to the same tool, returning out of order, cannot be
   reliably paired by a consumer. The passive contract requires preserved
   prose/tool relationships. Preserve the native correlation identity through
   the shared conversation shape and prove two calls with reversed results;
   do not infer pairing from tool name or source adjacency.

2. **P2 — An unrelated corrupt manifest takes down the whole Task feed.**
   `run_record.rs:582` reads and validates every Home manifest before checking
   Task attribution, and propagates any failure. A malformed manifest for an
   unrelated Run makes `lf task output LOO-293 --json` exit with no healthy
   output and only a JSON parse error. This also exposes a Run-directory
   creation race before its manifest is installed. Bounded discovery needs a
   deliberate partial-evidence contract: retain healthy Task output and expose
   discovery uncertainty explicitly rather than silently omitting evidence or
   globally failing every Task. Include malformed and disappearing unrelated
   records in its proof.

Both findings were reproduced with the real CLI over synthetic local sources.
They require decisions about shared identity and discovery evidence, so they
remain precise next-slice direction rather than ad hoc changes to the preserved
reader implementation during concurrent snapshot work.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Exact provenance | Actual immutable plan, repeated stages, Iterate/retry, settlement, atomic writes, parent silence | Existing ledger and owning transactions retain those facts | Fresh 8 store tests; source trace of all position-deletion and observation paths | pass |
| Resumable failure | No spurious failure/history when releasing a claim | Existing release behavior retained | Fresh 3 matching failure-release tests | pass |
| Passive output | Read exact recorded native sources and journals without controlling clients | Read-only Task dispatch, exact-account/source resolution, provider readers | Three configured CLI pages from `/tmp`; launch/write path inspection | pass for passive historical reads |
| Continuation transport | File/stdin outside argv | CLI file/stdin input and Swift private temporary-file transport | Fresh configured initial/file/stdin sequence; Swift source and prior transport receipts | pass |
| JSONL and mutable continuation | Partial lines, malformed records, bounded pages, revisions, boundary interleaving | Existing reader and OpenCode sweep logic | Fresh 7 reader tests | pass for tested cases |
| Tool relationships | Retain native call/result linkage | Call-side native IDs discarded | CLI synthetic two-call/reversed-result reproduction for Claude and Codex | gap |
| Partial discovery failure | Healthy output remains inspectable with explicit missing evidence | Unrelated malformed manifest aborts entire query | CLI synthetic unrelated corrupt manifest reproduction | gap |
| Complete capture | Native and autonomous prose, commands, results, failures before completion | Summary-only journal gaps and manifest-level provider labels remain | Source and prior dated evidence; no fresh Claude/OpenCode live proof | gap |
| Bounded history/live reads | Independent cursors and bounded discovery/state | One cursor; full manifest and Task-ledger scans; per-Run/boundary state grows | Source inspection | gap |
| Reset/compaction | Lost continuation is explicit | Replacement/count/boundary checks exist; arbitrary backdated/equal-count rewrites unproved | Reader source and prior ledger | gap |
| Wire shape | One ordered source array; Rust/Swift agree | Grouped records and explicit optional fields | Complete DTO/fixture diff inspection; prior fixture receipts only | pass by source and prior tests |
| Watch and complete demo | Diagram, attempts, all entry points, filters, Follow live, stale evidence, retained history | Absent at reviewed head; later snapshot work is in progress | Symbol search and TaskWorkspaceView's Changes/Terminal-only surface | gap |

## Configured and synthetic proof

Used the existing branch-built `target/debug/lf`, dated before the concurrent
snapshot edits, against `/Users/jack/.lf/loopflow.db` with explicit
`LF_CONTROL_HOME` and `LF_CONTROL_DB_PATH`, from `/tmp`. This is an existing-binary
configured receipt, not a fresh exact-head CLI build. Only counts were printed.

| Read | Records | Sources | Duplicate revisions | Gaps | Sources with history pending | Seconds |
|---|---:|---:|---:|---:|---:|---:|
| Initial | 197 | 3 | 0 | 0 | 3 | 0.366 |
| Cursor file | 166 | 3 | 0 | 0 | 1 | 0.331 |
| Cursor stdin | 128 | 3 | 0 | 0 | 1 | 0.318 |

491 distinct `(Run, source, item, revision)` tuples; no CLI stderr. Each returned
cursor was 1,887 bytes. History remained, so this is not fresh live arrival or
the configured Watch demonstration. An initial command without explicit Home
selection failed opening the absent development Home database. Selecting the
existing configured Home corrected the setup without modifying either database.

Reproduce the additional synthetic evidence with:

```bash
uv run python scratch/review-output-proof.py
```

The script keeps the actual Task registry read-only and points Run discovery at
a disposable directory containing only synthetic manifests and native JSONL.
Both providers returned four records with zero gaps but no preserved call-side
IDs. Adding one unrelated malformed manifest then produced exit 1, no stdout,
and `key must be a string at line 1 column 2`. No provider process was launched,
resumed, attached, interrupted, or replaced; no provider transcript or Task state
was modified. These synthetic sources establish normalization and discovery
behavior, not provider capture coverage.

Fresh focused verification:

- `env -u LF_RUN_ID LOOPFLOW_BUILD_PROVENANCE=development cargo test -p loopflow
  --lib durable_store_tests --no-fail-fast`: 8 passed.
- Reused that compiled test executable, `target/debug/deps/loopflow-19093d58746e5e6d`,
  with `LF_RUN_ID` unset: filter `run_record::output::tests::` passed 7; filter
  `failure_releases` passed 3. Reusing the executable avoided recompiling the
  concurrent snapshot edits into this review's proof.

The test build occurred in the shared checkout; it is not an isolated archive
verification. No new Swift tests, full affected suites, lint, or UI demo ran.
Prior receipts remain prior, and the 18 focused passes do not cover the two
newly reproduced gaps.

## Ownership and next slice

Negative architectural review found no provider launch/credential selection,
transcript writer, watcher cache, new table, or migration reachable from the
output query. Swift uses the typed CLI; it does not parse provider storage.
Launch/callback writers preserve source provenance beside Session identity.
Task flow facts remain excluded at both durable outbox and parent-notification
boundaries. The read projection never selects the next stage or reloads YAML.
The grouped source DTO has no reachable old-wrapper decoder or parallel feed.

The foundation advances the full architecture, but complete capture and bounded
discovery must be resolved before one-second Watch polling. Finish the shared
snapshot currently in progress, preserve the native correlation identities,
and design discovery failure evidence with independent history/live continuation.
Then complete the Mac surface and configured human demonstration. Summary-only
capture and unresolved provider attribution must not become an all-sessions claim.

The invoked [review-slice skill](/Users/jack/.agents/skills/review-slice/SKILL.md)
says: “When all applicable `Done when` claims hold and the slice is coherent,
publish or refresh the Task PR with `lf pr publish`.” That condition is not met
for this combined foundation or the newly requested snapshot. No publication,
landing, or Task completion was attempted.
