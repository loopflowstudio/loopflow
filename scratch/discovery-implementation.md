# Retained active Run discovery — 2026-09-24

Review follow-up: [the discovery review](review-discovery.md) reproduces and fixes
a manifest error surviving repair and client exit. The measurement receipts below
identify the preceding source; the review supplies focused proof of the correction.

## Finish line and current boundary

This is the Rust reader/native notification/CLI lifetime slice of
[the complete design](main-view-task-discovery.md). A foreground reader must
discover an existing waiting client without a new marker, follow publication and
removal, recover lost coverage explicitly, and perform zero historical directory
enumeration during unchanged warm observations. Passing a new-client-only fixture
or hiding a full scan behind a warm API does not meet that boundary.

The Rust slice is implemented and verified. All nine focused reader checks,
the actual CLI lifecycle proof, frame limit, Rust/Swift DTO and Monitor-state
checks, and the three-population cost/race matrix pass. Formatting, clippy with
warnings denied, and whitespace checks pass. [Final receipt](discovery-evidence/receipt.json).
Automatic Monitor transport remains the next internal slice. The Mac DTO and
empty-state consumer have migrated with the required discovery field, but the app
still uses its existing demand refresh. No automatic UI refresh is claimed.

## Implemented ownership and lifetime

`lf runs --active --watch --json` keeps one foreground Rust reader. Native receipts,
capture intervals, Exec receipts and the existing OpenCode registry remain the
ownership inputs. One-shot reads use the same cold discovery/projector. Warm
observations reread live/unresolved candidates and changed scopes; native matching
uses a PID index. No environment lookup, new receipt writer, durable index, daemon,
process owner or history reduction was introduced.

FSEvents subscribes before scanning and uses a separate callback queue. The
observation thread calls the SDK's synchronous delivery barrier, replacing the
design's proposed asynchronous barrier without blocking the callback on itself.
Cold scans take a second process sample after traversal; the final event boundary
rejects an overlapping ownership generation. Known receipts are reread after
projection, including in one-shot mode. Loss/coarse scopes and bounded-queue
overflow request an explicit cold recovery. Home directory replacement requires
a new reader so an old store cannot be paired with replacement receipt roots.

The command serializes observations, coalesces stdin refresh/rescan requests,
keeps one pending output frame and emits scanning heartbeats for long reads.
EOF/broken stdout cancels only the reader. Pipe threads belong to that foreground
process; scan cancellation checks entry batches. A blocked filesystem operation
may still require the owning Swift transport's planned terminate/reap fallback.
Persistent fatal errors wait for a request instead of repeatedly rescanning.
Continuous discovery is explicitly unsupported on Linux; one-shot remains supported.

## Counterexamples and review changes

- The first focused run failed the retained-capture/dead-native-client case:
  pruning dead native candidates turned a resolved ownerless capture into a
  missing-ownership warning. Ownerless bindings now consult their exact native
  receipt namespace and leave native liveness to independent native discovery.
  The original five ownership checks pass after that correction. The failure is
  retained in [the initial log](discovery-evidence/initial-ownership-failure.log).
- A second falsifying check then removed the last native receipt while its
  ownerless capture remained. The initial correction had dropped that binding
  from the cache, so retained discovery returned healthy empty while a cold read
  reported missing ownership. [Before](discovery-evidence/native-history-before.log).
  The reader now retains ownerless captures within its bounded unresolved set,
  caches native-namespace presence, and invalidates that fact on that Run's native
  events. Quiet observations reread only the capture, not dead native history.
  The regression also covers native restoration, client death and corrupt/repair
  recovery; the focused final reader run passes nine tests.
- The retained reader also owns Exec and OpenCode discovery inputs. Leaving
  `live_exec_providers` to reread those namespaces would conceal another retained
  history scan behind the native optimization.
- Review removed a per-receipt walk of all cached candidates when no unknown
  capture was being added. Unknown bookkeeping remains bounded; known/dead
  receipt processing does not pay that extra traversal.
- Review removed scanning frames from every ordinary warm refresh. Initial and
  explicitly requested recovery reads publish progress immediately; long reads
  continue heartbeat delivery. A shared condition variable cannot turn ordinary
  notifications into a heartbeat flood.
- The initial CLI/DTO test build exposed a private-module import in the tests.
  `DiscoveryState` now shares the existing public `runs` DTO export; production
  module visibility was not expanded to accommodate tests.

## Evidence and limits

- [Ownership/feed proof](discovery-evidence/ownership-feed-final.log): the five existing
  checks plus real native publication, old-directory resume, corrupt final
  receipt/atomic repair, receipt and Run-root removal/restoration, missing roots,
  replaced Home, cancellation, deterministic loss and queue overflow.
- [CLI proof](discovery-evidence/cli-final.log): actual foreground executable, owned
  waiting cat, automatic updates and explicit requests, scanning heartbeats while
  the real ownership-registry lock blocks completion, stdin EOF and broken stdout.
  The original client must remain alive after reader exit.
- [Swift proof](discovery-evidence/swift.log): required DTO decoding and Monitor's
  refusal to label scanning/unavailable snapshots as confirmed emptiness. This
  does not exercise a streaming Mac transport or native pane refresh.
- [Rust DTO](discovery-evidence/dto.log), [frame limit](discovery-evidence/frame-limit.log)
  and [static analysis](discovery-evidence/clippy-final.log) retain their exact commands'
  output. Final verdicts and source/artifact hashes are recorded after collection.

The [initial cost collection](discovery-evidence/initial-cost.log) passed all
three populations and sixty warm observations. Its instrumentation recorded
cumulative CPU and process peak RSS and only the final resume attempt. The pre-correction [expanded collection](discovery-evidence/cost-before-native-history.log)
adds per-observation CPU, current RSS, every resume attempt, publication
during a long scan, and cancellation of that scan. Both earlier matrices omit
an ownerless capture from their steady population and cannot validate the later
dependency correction. Their timed cancellation also arrived during process
sampling, before traversal. The final matrix includes that binding at every size
and uses a test-only first-directory signal for racing publication/cancellation.
It asserts that cancellation reaches an actual traversal. Its separately retained
executable and source hashes are in [the command receipt](discovery-evidence/measurement-command.json).
The [final report](discovery-evidence/report.md) records 60/60 warm observations
with zero directory enumeration, four receipt reads and two retained candidates
at all three sizes. Final measurement sources and its retained binary are unchanged.
These are local owned-process proofs, not configured-provider, compositor, human acceptance or full Task evidence.

No installation, publication, PM mutation, Task completion, provider transfer or
checkpoint of the concurrent flow contribution occurred. Its source and docs are
preserved. The full canvas's remaining composition, measurement, external-work and
chapter-integration obligations are unchanged.
