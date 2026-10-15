# Workflow loading — landing held

Jack Heart requires explaining and repairing the remaining delay before landing.
The 0.8-second stream measurement is not an accepted latency target or native
rendering proof. Trace query, transport, and Swift application/rendering separately;
verify the running demo binary. Cached Workflow must remain immediately usable.
Earlier evidence: 959a70fccc:scratch/workflow-loading.md. The subsequent land
request failed its CI repair timeout; no minor release started. This checkpoint
revokes the armed head through lf before further repairs.

## Remaining cost located

Current-source temporary instrumentation measured Task Process enumeration at
580–690 ms, Session/Flow membership at 5–21 ms, and Workflow reading itself at
0.06–0.19 ms. SQLite plans show the checkout membership branch scans Processes;
LOO-472 has 2,238 matching Processes. The header was waiting for diagnostic
history that only a collapsed Debug section consumes. Temporary stderr logging
has been removed (an intermediate publication checkpoint captured it; the final
repair removes it). No historical records were changed.

Normal Task frames now carry TaskOverview, without Process history. An explicit
`task_history` subscription carries the complete existing TaskWork while Debug
is expanded; closing it stops those queries, retaining cached readings. The same
Rust reader/snapshot/confirmation owners remain. No Swift database reader added.
Scope-to-frame measurements on real Tasks after this cut: LOO-462 86 ms,
LOO-466 6 ms, LOO-472 6 ms. These are not native render or agent-input timings.

The running demo PID3373 has the older CLI hash 273fd2ab3f7c949e..., whereas the
measured initial repaired CLI was 12816bd01725ecdd.... The app has NOT been
restarted or had its client drafts touched. Jack Heart was asked about a restart
before native measurement. New signposts separate Work-frame decode/application
and Task selection to the header observing Workflow data. The last endpoint is
a next-main-queue observation proxy, not compositor or physical input proof.

Review: diagnostic scope reattaches after reader replacement, closes on disclosure
hide, and rejects late data for another selected Task. Cache survives unavailability.
The full-history CLI stays unchanged. Existing trace endpoints cannot count
initial detail selection as completed Workflow rendering; the new interval is
separate. Landing remains held, verified OPEN with auto-merge disabled.

Checks: focused Swift 156/156 passed before timing instrumentation; final DesktopHeadless/DTO 60/60 passed after repairing the new history fixture, with the other 96 earlier passing tests unchanged. Rust stream 25 existing tests passed; the new diagnostic subscription regression passed after fixing its Wave/immutable-event/TraceId fixtures. DTO 42/42, final all-target Clippy, development CLI build, cargo fmt and diff check passed. Earlier actor-isolation instrumentation failure was repaired on MainActor. Logs: /tmp/loo462-overview-{swift-repair,history-final,dto-final,clippy-final,cli-final}.log. Native decode/application/render timings remain unmeasured pending demo restart; no native acceptance claimed.

October 15 demo restart: Jack Heart requested restart. Quit PID3373 normally,
replaced the demo's app executable and bundled lf with this checkout's checked
builds, ad-hoc signed/verified, and launched on /Users/jack/.lf with the canonical
Loopflow repo. CUA observed PID16133 with populated real sidebar and LOO-466's
Workflow header (No workflow yet), without a reading loader in that snapshot.
Its conversation was not auto-opened on restoration; the UI says to open it from
the sidebar. This snapshot is not a latency measurement or acceptance. Landing
remains held for Jack Heart's review and native timing.

October 15 review repairs: Jack Heart reported pink outlines on every file row
and sidebar selection disappearing during conversation opening, then highlighting
both Wave and Task. Navigator child rows now use plain buttons: their parent owns
keyboard focus and the selected file retains its selection edge. WorkNavigator
now highlights only selected Work, never Session membership (which can include
ancestors or be unavailable during opening). No membership or execution changed.
Check: TaskFilesTests 22 PASS (optional native diagnostic skipped), WorkNavigationTests
41 PASS, builds and diff check PASS; /tmp/loo462-{files-focus,sidebar-selection}-tests.log.
Review: removed the legacy Session-driven selection branch rather than introducing
loading-specific selection state. Current demo has not yet been restarted for these
latest two visual repairs; native confirmation remains.
