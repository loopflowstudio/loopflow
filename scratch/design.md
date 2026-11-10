# LOO-802: Typed subscription events

Add `event {id,seq,event}` beside complete subscription readings so clients can
react to a Task shipping, an arriving steer or a recorded Graph step exit.
Revision 4 is the current protocol; the brief's revision-2 wording predates it.
Reconnect restores current state without replaying reactions or adding a replay
cursor. Board animations and thread UI consumers remain outside this Task.

Jack Heart, Task comment 0b858325-ce38-42cc-941e-581a734994db:
> Run this Task through auto entirely with Codex: implementation, review, QA, CI repair, merge and completion. Base PR #1813 has merged. Other follow-ups run in separate checkouts; keep this Task scoped to its brief, sync before delivery, and preserve their changes. Resolve routine implementation choices autonomously. Report a missing product decision or external dependency rather than broadening scope.

## Implemented contract

Existing PR merge records, immutable steers and Graph step exits supply
occurrences. The first reading baselines history in its pinned snapshot.
Selection cannot hide events; unavailable membership retains the previous
scope. A departing Task delivers its final observed occurrences; newly included
Tasks do not replay history. “Shipped” means a recorded PR merge, and step
completion includes failed and interrupted exits.

`Subscription::pull` returns one `SubscriptionUpdate` containing the changed
reading and its occurrences. The separate event drain, shared buffer and
frame-only wrapper are gone. Readings and events share Graph-step conversion.
No migration or parallel event log is needed. Delivery uses the existing bounded
reservation; sequence assignment follows cancellation filtering and spans the
connection. Overflow ends the subscription so it can reopen from current state.

Swift event consumers receive readings and events in one ordered stream and
reconnect with a fresh reading. Sequence gaps end every event consumer on the
connection; unknown kinds end only the affected consumer. Reading-only watches
retain checkpoint resumption. Occurrence IDs deduplicate content already in a
reading or delivered through overlapping subscriptions; sequence numbers order
delivery, not occurrences.

The shipping contract and examples live in
`rust/loopflow/src/server/README.md`, linked from `docs/dev/server.md`.
`scratch/questions.md` records the interpretation choices; none needs a product
decision. The concept walkthrough is `scratch/concept-review.html`.

## Concept review — 2026-11-09

Outcome: advance; no design change or open product choice. The live comment,
lost delivery and fresh-baseline recovery fit the existing contract. The main
consumer trap is appending a comment twice when its reading precedes its event:
deduplicate content by comment ID while treating the event as a presentation cue.
Overlapping subscriptions likewise need occurrence identity, not delivery order.
The departing-Task and newly-included-Task counterexamples require no new model:
deliver the former's final observed ship and baseline the latter quietly.
Existing usage docs already describe this; no skill changes are needed.

The next demonstration remains a real recorded merge leaving a plan. Store-level
coverage supports that contract, but the recorded live QA did not exercise it.
Board animations and thread presentation in the concept page are illustrative,
not built UI or additional implementation direction.

## Remaining

- Review and affected-suite gate, then repair any failures. No review submission
  was reported for PR #1873 by `lf pr reviews` on 2026-11-09.
- Sync before delivery if main advances, publish the final PR content, repair CI
  if needed, and land PR #1873.
- Verify merged behavior before completing the Task. The live QA did not exercise
  an actual GitHub ship leaving a plan; the observer test covers that scenario
  at the store level. A real recorded merge remains a verification opportunity.
- Mounted Desktop presentation is unverified and new UI consumers are out of
  scope. Automatic ServerClient recovery, gaps, unknown kinds and saturation
  were not exercised by the live walkthrough; focused tests cover recovery,
  gaps, unknown kinds and observer overflow. Gate/CI own the remaining automated
  checks; the headless run does not establish mounted behavior.

Gate's Rust filter is
`test(server::) | test(lf::commands::subscribe::) | test(store::sqlite::task_comments::) | test(dto_fixtures::) | binary(served_reads)`;
Swift, website/architecture and clippy also run through the affected-suite runner.
No successful affected-suite gate is recorded here.

## Recorded checks and observations

QA rerun, 2026-11-09, `d5bba3db5`: CLI and Swift library builds passed;
`uv run python scratch/qa_events.py` passed all ten live scenarios again.
Swift decoded 16 readings, 20 steer events and two Graph step events. Seven
admission refusals were retried before all eight comments arrived on both
plan and dashboard watches. Both private server children were reaped, no
private terminal owners were found, and the final process check found no QA
process remaining. `scratch/qa.html` and its transcript contain this run.
No production repair was needed; live ship, mounted Desktop, automatic client
recovery and maximum-load limits remain as listed above. Gate/CI own the
affected-suite checks. The report now records the measured checkout revision.

Realign, 2026-11-09: snapshot collection, cancellation/sequence assignment,
bounded delivery and Swift recovery agree with the contract above. No code
repair was needed. Fetched `origin/main` is `7cf2d25b3`; this branch at
`d5bba3db5` is 0 behind and 3 ahead, including base PR #1813 and reading-checkpoint
resumption (#1865). Task status reports the current auto Graph at realign;
the earlier Graph stopped at gate. No Wave is attached.

Existing focused results apply to unchanged production code:

- `cargo test -p loopflow --lib -- server::events:: server::outbox:: server::service::tests::typed_events lf::commands::subscribe::`: 16 passed after the single-pull API cut; macOS linker emitted an `__eh_frame` size warning.
- `cargo test -p loopflow --lib server::`: 40 passed before the final payload buffer-drop adjustment; gate covers the final code.
- `cargo test -p loopflow --lib server::events::`: 7 passed.
- `cargo test -p loopflow --lib server::service::tests::typed_events_share_connection_order_survive_selection_and_stop_on_cancel`: passed.
- Built library test binary, `server::wire::tests::` and `server::schema::tests::`: 2 and 7 passed respectively.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'ServerProtocolTests|ServerClientTests.*(eventWatchReconnects|eventRecovery|disconnectedWatchResumes)'`: Rust/Desktop build and 20 Swift tests passed.
- `cargo build -p loopflow --bin lf`: passed for live QA.
- `swift build --package-path swift --target Loopflow -Xswiftc -gnone`: passed for the wire probe.
- `uv run python scratch/qa_events.py` with the compiled Swift probe: passed; private servers and terminal owners were stopped.

The 2026-11-09 live walkthrough (`scratch/qa.html`, transcript at
`scratch/qa-transcript.jsonl`) passed ten grouped scenarios: released Session
state, comment payloads, overlapping/selected watches, cancellation, real Graph
exits, disconnect recovery, CLI agreement, plan/dashboard bursts, empty input,
server restart and Swift wire interoperability. Swift decoded 16 live readings,
20 steer events and two step events. No typed-event defect was found in these
flows. Maximum-load and slow-reader fault injection remain outside this result.

QA also found that Task checkout warmed primary conversations after command-only
Graphs. All owners and descendants tied to disposable checkouts were stopped;
the driver and QA skill now isolate provider homes and clean up those processes.
The full walkthrough passed again with no private QA lf process remaining.

The docs preview command
`lf open --host desktop --file rust/loopflow/src/server/README.md --file docs/dev/server.md`
returned “Error: task_82f1213c2bdf4537a349942b6f1a7939 has no workspace in this Desktop window; nothing changed.”
The docs remain readable at those paths; this does not establish a mounted preview.

- Sync conflict check, 2026-11-09, `fcd75a311`: `cargo test -p loopflow --lib server::service::tests::typed_events_share_connection_order_survive_selection_and_stop_on_cancel` (with `LF_*` unset and stdin closed) passed, 1 test; macOS linker warned about `__eh_frame` size. Gate/CI own broader checks and clippy after the sync.
