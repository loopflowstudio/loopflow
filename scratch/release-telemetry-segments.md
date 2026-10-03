# Retain telemetry obligation segments

## Change

The existing obligation document now represents an `ObligationSegment` for
release or telemetry jobs. It retains the same calendar, placement, observation
and successor fields. Only release jobs materialize release opportunities.
There is one storage owner and atomic replacement path; no telemetry scheduler,
execution ledger, additional timer or independent settlement writer was added.

Cron sync retains the installed predecessor before replacement. If it already
has an observed timezone, that evidence survives unchanged. Legacy installations
without a record retain their activation but begin timezone observation now.
Moving to a different repository or Home closes the previous segment in its
original store. Removal likewise retains and closes the installed segment before
unloading. Telemetry execution and preflight failure also observe its segment.
Unchanged sync retains the same identity and observation frontier.

Each frozen original prerequisite now includes an optional `obligation_id`.
Its due interval comes from the segment active on the release's Home at that
original release due, using that segment's schedule and timezone. Receipt joins
also match the original repository, Home, job, schedule and scheduled source;
the interval ends at the earlier of the next due or segment closure. A moved
Home or replacement schedule cannot lend its receipt to the old interval.
Repository aliases resolve to the retained canonical identity.

Missing segments, gaps after removal, dates before observation and releases
before a new segment's first telemetry due remain explicitly unknown. The last
case conservatively does not borrow a previous segment's check across a change
between telemetry and release. This decision is recorded in `questions.md`.
Current recovery policy, its single reservation and bounded wait are unchanged.

Release history exposes telemetry segments alongside release segments. Their
empty opportunity lists contribute no release dues, executions or settlements.
The release observation frontier still derives only from release segments.
The JSON fixture includes the reference and its telemetry segment; existing
qualification and failure-disposition expectations remain unchanged.

## Proof and review

Two focused segment cases exercise schedule changes, UTC/Eastern timezone
changes, Home changes, removal gaps, unobserved cutover and a receipt arriving
after segment closure. They retain original failed receipt bytes and reject a
new Home's otherwise matching receipt. Their first run exposed filesystem alias
matching: temporary-repository receipts can name `/var/...` while retained
segments name `/private/var/...`. The production join now recognizes canonical
identity; the cases passed afterward. This was not a production release failure.

A real file-backed installation case starts with a legacy plist and no retained
segment, replaces its schedule, repeats unchanged sync, moves Home storage and
removes the job. It verifies the retained predecessor, stable current record,
closed original-Home records and closed replacement without release opportunities.
Launchctl is simulated; no production job is installed or removed.

The joined built-CLI fixture now spans two retained telemetry schedules. A failed
receipt from the older schedule remains linked outside the history display
window; the current check and product outcome remain distinct. It verifies both
segment references, frozen catch-up coverage and exact caller preservation.
Git, CLI dispatch, registry and OS locks are real; telemetry and external release
services are simulated, and due times/segments are seeded. This is not configured
calendar acceptance, actual doctor/scorecard verification or public/UI proof.

Review checked the lifecycle writer, segment selection, history counters and
fixture mirrors. The predecessor must be captured before replacement, including
legacy jobs and moves to another store; merely observing the new job would lose
that evidence. Telemetry must not materialize release opportunities or move the
release observation frontier. The implementation covers these boundaries.
No old receipt is rewritten and no descriptor supplies new process authority.

## Remaining work

Actual doctor/scorecard continuity, the observed missing `agent_turns` scorecard
blocker and its unaccepted Intelligence handoff remain. All 36 retained telemetry
failures, including the original 35, remain counterevidence. Dated repair ownership,
closed unfinished release continuation, exact overlap continuation and remaining
interruption proofs stay in this serial Task. Cross-Home records remain in their
own stores; this does not transfer authority or provide a fleet-wide history join.
Required UI/public proof, supported cutover and two adjacent real automatic
settlements (at least one publication, no manual repair) are still open.

No full gate, hosted matrix, production installation/sync, trigger, publication,
PM handoff, PR publication, landing or Task completion occurred.

## Validation

- `cargo test -p loopflow --lib ops::cron::accounting::tests::telemetry_ -- --nocapture`: three passed, 0.20s. This final behavioral pass follows the canonical-identity repair; later changes only clarify uncertainty/error text.
- `cargo test -p loopflow --lib telemetry_installation_retains_legacy_replacement_move_and_removal -- --nocapture`: one passed, 0.16s.
- `cargo test -p loopflow --lib ops::cron::history::tests -- --nocapture`: four passed, 0.01s, including fixture round-trip and unchanged settlement/failure counts.
- `cargo test -p loopflow --test scheduled_release_tests historical_telemetry_segments_survive_schedule_replacement_in_release_history -- --nocapture`: passed, 27.03s (2m49s build including lock wait). The preceding run also passed in 23.04s; the final run includes bounded fixture-path enumeration before adding its predecessor document.
- `cargo clippy --all-targets -- -D warnings`: passed, 1m36s including build lock wait.
- `cargo fmt --check` and `git diff --check`: passed.

The initial broad `telemetry_` name filter selected fourteen tests: twelve passed
and the two new segment cases failed on the alias join before repair. No full
suite or configured acceptance is inferred from these focused results.
