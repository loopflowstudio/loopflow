# Active Run discovery across observations

Design and implementation ledger, 2026-09-24. The Rust reader/CLI slice is verified;
automatic Monitor transport is now enabled with [focused local proof](active-runs-stream.md). [Implementation and evidence](discovery-implementation.md).
[Review](review-discovery.md) corrects manifest-error lifetime with focused proof;
the cost matrix remains attributed to its recorded pre-correction source.

## What to build

Keep one foreground Rust active-Run reader alive beneath each Podium Home reading,
discover existing ownership receipts once, then follow filesystem changes and
revalidate processes so every Monitor updates automatically without repeatedly
enumerating retained Run history.

User seed: “Design the next discovery slice from scratch/review-scroll-discovery.md
and scratch/active-run-refresh.md.” Governing constraints: “Keep the existing native
receipt as authority” and “one transient long-lived reader with filesystem change
notifications and explicit cold/rescan cost.”

This is an indivisible part of LOO-291's existing canvas change: internal slices,
one PR. Receipt discovery, subprocess lifetime, recovery, shared refresh and their
cost proof ship together. A cache inside repeated one-shot commands is insufficient.

## Placement and boundary

Product Wave, existing LOO-291 and its current internal chapter Project. Existing
handoff identifies Desktop historically; this design neither selects a new Project
nor assumes live chapter migration. No planning mutation or new Task is needed.

Implement the notification backend for the local macOS desktop first. Existing
one-shot active reads remain supported on Linux; continuous mode there reports
unsupported until a backend has its own race/resource proof. This is an explicit
platform boundary, never a polling fallback. Linux inotify requires watches on
individual directories, so a recursive implementation would introduce a separate
history-dependent watch-resource cost. [Linux API](https://man7.org/linux/man-pages/man7/inotify.7.html).

## Current system and replacement

`run_record/active.rs::snapshot` enumerates `record_dirs`, reads native receipts,
reads capture bindings before/after process observation, and resolves shared Work.
`top.rs::live_exec_providers` samples processes but also reloads Exec receipts and
the OpenCode registry. Native matching currently loops over processes × clients.
`RegistryQuery.activeRuns` launches a fresh command; Podium shares its result but
only refreshes on demand. Pane layout and terminal ownership already have owners.

Separate receipt discovery from process sampling within these existing readers.
Keep one ownership join and one Work resolver for cold and continuous modes.
Index native candidates by PID before applying the existing PID/start/harness
checks; retain all matching candidates so ambiguity cannot be hidden by the index.
Read live manifests and resolve Work on every observation, avoiding a second cache
invalidation protocol for chapter membership or attribution.

Delete Monitor's repeated one-shot transport after integrating the continuous
reader. Keep the public one-shot CLI as a cold invocation of the same discovery
and projection code. Do not retain two independent algorithms.

## Lifetime and transport

Add `lf runs --active --watch --json [--task …]`. It remains a foreground read
process: no daemon, socket, installation, resident Wave, durable cursor or service.
Stdout is newline-delimited complete `ActiveRunsSnapshot` values; flush each frame.
Stderr carries diagnostics. Task filtering follows Home-wide discovery, not a
separate per-Task cache. One-shot output remains one JSON document.

Podium starts one unfiltered stream on first Monitor demand and retains it until
window teardown or Home/helper configuration replacement. Task/repository changes,
closing a Monitor and switching to a terminal do not restart it. This deliberately
spends one small reader per used window to avoid repeated cold scans on navigation;
other windows have their own Podium lifetime. No application-global registry is
introduced. All panes in that window share the same observation.

RegistryQuery owns typed transport access; RegistryQueryLocal owns the child and
pipes, using LocalWaveAgentLauncher's existing helper/environment preparation.
Capture resolved Home, database and helper configuration at start. Reject frames
from a mismatching Home. Configuration replacement cancels and awaits the old
reader, clears old-Home evidence, then starts a new generation. Late callbacks
cannot publish into that generation. Repository selection alone is only filtering.

Sample every two seconds, serially, with no queued catch-up ticks. Notifications
accumulate dirty paths; they do not launch parallel observations. Refresh sends
`{"action":"refresh"}` on stdin and coalesces with the next serialized observation.
This tiny request enum is read control, not Work orchestration. EOF, cancellation,
closed stdout or window teardown releases the watcher and exits; Swift terminates
and reaps only its owned reader if graceful cancellation does not finish. No
provider receives a stop signal.

Drain both pipes off the main actor. Bound pending output to one latest snapshot;
never buffer an unlimited stream of observations. Swift also keeps only the latest
decoded delivery. A 16 MiB frame limit is a transport resource limit: exceeding it
reports unavailable, never truncates Runs into a healthy result. Exit, malformed
JSON, or ten seconds without a frame marks the reading stale/unavailable. Scans
emit progress-state heartbeats, without inventing observation times for old data.
Transport failure stops automatic restart churn; Retry starts one fresh reader.

## Values and functions

Keep `ActiveRun`, native client receipts, capture intervals, Exec receipts and
`WorkRef` unchanged. Add required `discovery: scanning | ready | unavailable` to
Rust/Swift `ActiveRunsSnapshot` and migrate every fixture/consumer together.
`ready` means discovery coverage is established; process/attribution errors still
appear in `gaps`. It does not mean the snapshot is an atomic OS/filesystem image.

Private `ActiveRunReader` holds the selected Home, OS subscription, retained
live or unresolved ownership candidates, a coalesced dirty-path set, and recovery state. The serial CLI supplies the existing store. Definite dead PID/start identities are discarded from memory, never
deleted on disk. A later resume is discovered by its existing receipt publication.
Ownerless capture bindings remain bounded unresolved candidates even when native
history currently resolves their attribution. Cache that namespace presence and
invalidate it on the Run's native changes; deleting this dependency would miss
loss of the last native receipt. Quiet reads recheck the capture without rereading
dead client history. Unknown observations retain candidates and gaps. No complete historical directory
tree or set of dead receipts is retained after scanning.

Proposed seams, with existing error types where applicable:

```rust
ActiveRunReader::start(home: &Path, continuous: bool, cancel: CancellationToken) -> Result<Self>
ActiveRunReader::invalidate(&mut self) // next observation performs explicit recovery
async ActiveRunReader::observe(&mut self, store: &SharedStore, task: Option<WorkRef>) -> ActiveRunsSnapshot
```

```swift
RegistryQuery.watchActiveRuns() -> ActiveRunsObservation
// Cancellable transport handle: snapshots, requestRefresh(), cancel().
PodiumModel.observeActiveRuns() async
```

The handle owns transport only; Podium owns freshness and last-good presentation.
Scanning/unavailable frames retain that last-good reading with a visible reason.
Only a ready snapshot with no relevant gaps permits confirmed emptiness. Partial
ready observations can expose their proven Runs with incomplete evidence labeled.

## Subscription, publication and recovery

Use macOS FSEvents with file events and root-change reporting, started before
enumeration. Subscribe at the existing Home ancestor needed to observe ownership
roots even when absent: `runs`, `run-bindings`, the existing Exec-receipt root and
`runtime/opencode-servers.json`. Filter unrelated paths before queueing, but process
loss/root flags before filtering. Watchers and scans create no missing directories.
The existing registry file is reread only when changed; its full rewrite cost is
accounted separately. No additional provider writer is added.

Cold discovery streams directory entries instead of collecting all Run paths.
Load native receipt files and their manifests, capture bindings, Exec receipts and
the existing server registry. Resolve ownership against one process sample and
retain only relevant candidates. Skip writer staging files; only published final
names supply evidence. A missing file after enumeration means it was removed;
malformed committed content or inaccessible traversal remains a gap.

During scanning, accumulate changed scopes. After enumeration, use
`FSEventStreamFlushSync` from the foreground observation thread, with callbacks
on a separate dispatch queue. The SDK guarantees preceding delivery at return;
the callback never waits for its own queue. This replaces the proposed async
barrier with the native synchronous barrier on a worker thread. Reconcile queued
paths and repeat until that boundary is caught up. Rescan changed directories
even if they were already visited. A newly
created or moved-in subtree must be enumerated; atomic replacement must replace
the candidate. Parent renames invalidate both cached descendants and discovery
coverage. Do not derive ownership directly from an event payload.

For each warm observation: flush pending delivery, sample processes once,
reconcile dirty paths and retained receipts against that sample, project exact
ownership and Work, then reread known receipts and flush/check again. Cold scans
add a fresh second process sample after traversal so a long scan cannot publish
its initial process state. Publications after the first sample remain in the
notification queue and invalidate that observation. An ownership change overlapping the
sample invalidates that result; schedule the next observation with a gap instead
of mixing generations. Quiet output/transcript changes do not invalidate ownership.
Changes after the final boundary belong to the next observation; this is eventual
observation, not a transactional or lossless Run-history feed. Brief clients that
start and exit between samples may never appear.

Apple requires monitoring before scanning and rescanning affected subtrees on
coalescing; dropped events require full recovery. Root changes and event-ID wrap
also invalidate coverage. [FSEvents guide](https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/UsingtheFSEventsFramework/UsingtheFSEventsFramework.html).

Latch `scanning` before recovery. Clear suspect coverage, re-establish subscription
where necessary, then rebuild using the same cold algorithm while collecting new
events. Emit no healthy empty during recovery. The bounded path queue holds at
most 4,096 entries/4 MiB; overflow becomes one full-rescan request, never silent
eviction. Limit unresolved-candidate bookkeeping to the same bounds; exceeding
them leaves discovery unavailable rather than claiming completeness. Live
candidates are not silently capped. Persistent permission/watch failure ends
recovery as unavailable and offers Retry; continuous churn retains incomplete
observations instead of claiming convergence. Cancellation interrupts scan batches.

On sleep/wake or lost subscription continuity, explicitly invalidate and rescan.
Podium forwards the existing macOS wake notification as the request enum's
`{"action":"rescan"}` case; the reader also invalidates on OS root/loss signals.
The reader checks the canonical Home directory identity. Removal/replacement
reports unavailable and requires a fresh reader to resolve Home/store together;
it never silently keeps the old database beside replacement receipt directories. Event cursors
are process-local and discarded at exit; no replay claim spans offline periods.
Native FSEvents binding must preserve loss flags and delivery barriers. A wrapper
that erases either is unsuitable. Backend health and filesystem support are proof
obligations, not permission to silently substitute repeated full scans.

## Cost contract and measurement

Let H be retained receipt/history entries, P observed processes, L retained live
candidates, U unresolved candidates, and D changed ownership entries/subtrees.
Cold/recovery work is explicitly O(H + P); ordinary warm discovery visits L, U
and D, never H. Ownership matching should be indexed rather than P × H. Ancestor
lookup and Work resolution retain their existing cost and must be measured.
Coarse subtree invalidation can cost O(H); record it as recovery, not a warm win.
The server registry's rewritten bytes count in D. Memory includes P + L + U and
the bounded event queue; report peak cold and steady RSS separately.

Instrument test/benchmark counters for directory visits, receipt/manifest reads,
bytes, process samples, retained candidates, dirty paths and recovery reasons.
Exercise 100, 10,000 and 100,000 old Runs with the same owned waiting clients,
stale native receipts, stale bindings and Exec receipts. Compare current one-shot
reads with cold, twenty unchanged warm, single-publication and forced-loss reads
on identical populations, measurement source and host; record each implementation's
source/binary separately. Unchanged warm samples must enumerate zero
historical Run directories and reread zero dead receipts at every size. One new
receipt must inspect its scope, without a full scan. Measure wall time, CPU, RSS
and mutation-to-visible latency; preserve failures and separate recovery samples.
Two seconds is a proposed cadence, not a latency budget or measured result.

## Demo and Done when

Start an old owned waiting client before launching the reader, with no environment
locator or new capture binding. Open two Task Monitors beside a retained Session
and companion terminal. Both show the correct shared observation. Publish a new
client, resume an old Run, and exit clients without receipt cleanup: rows update
automatically under exact Task identities, while drafts, focus and PTY replies
survive. Force notification loss: show stale/recovering evidence, then converge to
an independent complete cold read. Close the window: its reader exits and externally
owned provider clients survive. Embedded terminal teardown retains its existing
window semantics. Configured provider proof uses newly owned identities;
local cat fixtures remain explicitly local proof.

Required falsifying cases:

- Preserve all five restored ownership tests: old cat clients, same-checkout Task
  isolation, PID reuse/death, deduplication and sequential Runs within one Exec.
- Publish/remove/rename during subscription, traversal and process observation;
  resume inside an old directory; start with missing roots; repair corrupt final
  receipts; delete/recreate roots; inject dropped/coalesced events and queue overflow.
- Real FSEvents delivery proves publication/replacement/removal. Deterministic
  loss injection proves recovery; passing only a fake notification source is
  insufficient. Full scans occur only for cold start or recorded invalidation.
- Two panes share one reader. Task/repository return does not restart it. Home
  replacement rejects late frames. Failed reads, stalled/broken pipes and child
  exit retain explicit last-good evidence. Cancellation leaves no orphan reader.
- Rust/Swift fixtures agree, both native and fallback builds compile, and mounted
  native input survives automatic updates and recovery. Run the cost matrix
  before enabling automatic refresh in the integrated product.

## Internal slices and forbidden outcomes

1. **Completed Rust slice:** reader, native subscription and CLI lifetime; races,
   cold/warm costs and recovery verified using owned processes. See the
   [implementation receipt](discovery-implementation.md).
2. **Completed Swift slice:** cancellable RegistryQuery transport and one Podium
   subscription now drive automatic updates. Refresh, Retry, wake/rescan, scope
   replacement, teardown and retained native input have [local proof](active-runs-stream.md).
3. **Next slice:** demonstrate positive configured Run changes and recovery beside retained panes;
   record exact sources, binaries and cost results for review.

No environment-only lookup, recent-history window, new-client-only index, durable
Watch model, alternate ownership registry, per-pane subprocess/poller, history or
transcript reduction, implicit provider action, healthy empty after evidence loss,
or quiet fallback to history scanning. Caching stale receipts for every historical
Run would merely exchange the repeated scan for unbounded retained state.

Remaining canvas work keeps its existing scope: compositor/hitch and phase
measurements, configured costs/budgets, objective/target integration, human
composition acceptance, external trials and the authorized edit. No optimization
Tasks are filed, and this discovery proof cannot complete LOO-291 or LOO-293.

## Evidence and assumptions

Observed: the rejected environment locator omitted an owned cat client; two Rust
ownership cases failed. Restored discovery subsequently passed all five cases.
The supplied [review](review-scroll-discovery.md) and
[counterexample](active-run-refresh.md) remain the evidence. Source inspection in
this design confirms one-shot transport, retained-directory traversal and repeated
supporting ownership reads. That statement describes design-time evidence; the implemented reader and final
benchmark are now recorded in [the implementation ledger](discovery-implementation.md).

Selected defaults: macOS continuous mode first, one reader per Podium/window/Home,
retained after first demand, two-second sampling, explicit Retry after transport
failure. FSEvents correctness under the actual local Home and cancellation during
scan are implementation proof obligations. If they fail, revise this design before
automatic refresh; neither a historical receipt nor this proposal proves them.
