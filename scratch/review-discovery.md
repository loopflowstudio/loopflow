# Retained discovery review — 2026-09-24

The Rust reader advances the accepted discovery design. Review reproduced a stale
manifest diagnostic and removed it from retained discovery state. Automatic shared
Monitor transport remains the next internal slice; this is not full canvas acceptance.

## Finding and correction

A live native client's malformed manifest produced a cached error. If the client
then exited, receipt pruning removed the only candidate that could clear that
error. Repairing the manifest left the retained reader reporting unavailable
activity indefinitely, even though an independent cold read returned cleanly.

The new `manifest_failure_does_not_outlive_its_native_client` regression fails
before correction: retained discovery has the old parse error; the cold result has
no gap. [Failure](discovery-review-evidence/manifest-before.log).
Manifest failures now travel with the current observation's live-client results,
not the receipt-discovery cache. Receipt/traversal errors retain their existing
recovery path. No new reader, writer, DTO or persistent state is introduced.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Old waiting clients | Discover clients without new markers or bindings | Existing native receipts and exact PID/start/harness validation | Original ownership tests and real foreground CLI with owned cat | pass locally |
| Changes and recovery | Observe native publication, replacement/removal and explicit loss recovery | Subscription precedes scan; changed scopes reconcile; overlapping ownership returns scanning | Native feed regression, cold comparison, prior publication/cancellation matrix | pass at recorded scope |
| Diagnostic recovery | Repaired/irrelevant evidence cannot leave a permanent warning | Manifest errors are derived per observation | New failing-before regression and corrected focused run | pass after correction |
| Reader lifetime | EOF/broken stdout exits reader without stopping providers | Foreground stream, bounded output, cancellation and scanning heartbeat | Actual CLI test; real registry lock holds scan while heartbeats continue | pass locally |
| Bounded warm discovery | Cost follows live/unresolved/changed entries, not historical population | Transient candidates and event invalidation; same cold projector | Hash-verified prior 100/10,000/100,000-Run matrix: 60/60 quiet reads enumerate zero directories | pass for recorded pre-correction source |
| Shared wire and emptiness | Scanning/unavailable cannot appear as confirmed empty | Required Rust/Swift discovery field; Monitor checks ready plus gaps | Unchanged DTO/Swift source hashes and prior focused receipts | pass at fixture scope |
| One automatic Mac reading | One cancellable transport shared across panes with late-frame rejection | RegistryQuery still starts one-shot reads; Podium remains demand-refreshed | Current production source | next-slice gap |
| Complete acceptance | Both build paths, automatic native input retention and configured positive activity | Prior scoped evidence only | Current design/directive and recorded limits | gap |

## Focused verification

`cargo test -p loopflow --lib run_record::active -- --nocapture` passes ten
checks; the explicit large cost matrix remains ignored in this focused command.
[Corrected reader log](discovery-review-evidence/reader-after.log).
`cargo test -p loopflow --test active_runs_watch` passes against the corrected
reader: [final CLI log](discovery-review-evidence/cli-after.log). Its earlier
pre-correction run also passed and remains a separate receipt.

`cargo clippy --all-targets -- -D warnings`, `cargo fmt --all --check` and
`git diff --check` pass. No executable edits followed these checks. The
[review receipt](discovery-review-evidence/receipt.json) records final source and
artifact hashes; the [bounded patch](discovery-review-evidence/correction.patch)
is separate from concurrent implementation work. Existing unchanged Rust/Swift DTO
and Monitor receipts were inspected, not rerun. No broader suite, fallback build
or new benchmark is claimed.

## Review boundaries

Read the cached LOO-291 directive through `lf pm show --wave product --no-sync
--json`, the governing workspace design, the complete discovery design and prior
scroll/discovery review. The directive requires bounded native discovery and
shared automatic refresh in this core PR; neither is an optimization follow-up.

The whole-Task `lf task diff` hit its 1 MB cap. Recovered the 810-file inventory
through `lf task changes` and 214 narrower `lf task diff` calls, including
untracked files. One 1.1 MB newly added historical roadmap fixture still hit the
per-file cap; its complete contents were read and parsed separately (its base is
empty). All 810 sections are accounted for: 670 byte-identical to the prior
scroll review, 140 changed/new, none absent. The complete chunk index is
`/tmp/loo291-discovery-review-patches/index.json`; the durable
[scope receipt](discovery-review-evidence/diff-scope.json) preserves accounting.
Capture overlapped addition of the regression; the final correction has its own
patch and hashes. Reused prior reviews for unchanged source and inspected this
slice's source and integration seams. Concurrent Task-bound flow changes remain
their writer's contribution and are not approved or checkpointed by this discovery
review.

Reviewed receipt classification and pruning, native event filtering/loss flags,
subscription/scan ordering, process sampling, exact ownership joins, attribution,
CLI argument routing, serialized cadence, pipe backpressure/cancellation, DTOs and
Monitor emptiness. The installed SDK's FSEvents header documents synchronous
callback delivery for FlushSync; the implementation calls it off its callback
queue and drains that queue before releasing its boxed context.

Negative searches retain one production capture-binding writer, existing native
receipt publication, one shared active projector, one Podium active-Run caller and
one root native workspace registry. Active discovery has no `record_dirs` fallback,
environment locator, durable cursor/index, provider stop action or per-pane reader.
Historical Run readers still enumerate history for their existing history APIs;
they are not reached by the retained active loop. Exec and OpenCode evidence are
held by the same reader rather than silently rescanned every tick.

The existing benchmark artifacts and all 19 recorded source hashes matched at
review entry. The bounded manifest correction changes reader/test source, so the
prior matrix remains attributed to its recorded executable. It is not relabeled
as a benchmark of this revision. The correction changes diagnostic lifetime and
adds no receipt reads, directory traversal or discovery algorithm. No broad or
performance rerun is warranted for this error-path fix.

Cold discovery remains expensive (about 50 seconds at 100,000 old Runs in the
recorded debug run). The stream must expose scanning and the Mac consumer must
retain useful last-good evidence while recovering. Blocking filesystem/registry
operations still require the planned owned-child terminate/reap fallback at the
transport boundary. The local cat and temporary-Home proofs do not establish
configured vendor-provider behavior or human acceptance. Linux continuous mode is
explicitly unsupported; Linux one-shot support remains separate from native proof.

## Next slice and disposition

Connect the foreground stream to cancellable RegistryQueryLocal transport and one
Podium observation per window/Home. Drain pipes off the main actor, reject old-Home
and late-generation frames, retain last-good evidence, forward wake/rescan, and
terminate/reap only the owned reader when graceful cancellation stalls. Remove
Monitor's repeated one-shot transport. Prove two panes share updates without
restarts on Task/repository navigation, and preserve native drafts, focus and
responding companions during updates, recovery and window teardown.

No publication or Task completion. The invoked
[review-slice skill](/Users/jack/.agents/skills/review-slice/SKILL.md) requires
“When all applicable `Done when` claims hold and the slice is coherent” before
publication. The design explicitly ships discovery, shared transport and recovery
together; automatic Mac integration, configured proof and full acceptance remain
open. No installation, PM mutation, provider transfer or checkpoint of concurrent
flow changes occurred.
