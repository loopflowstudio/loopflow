# Harnesses

Each provider (`claude.rs`, `codex.rs`, `opencode.rs`, `pi.rs`) gets launched,
observed and recorded here. The provider owns the conversation; lf adds context
at turn boundaries, interjects through hooks and records what happened.

A skill keeps one `LfProcess` across successive provider `AgentProcess` instances.
Its shared `LfProcessRuntime` in [`process/runtime.rs`](../process/runtime.rs)
holds the current attachment, provider, account, thread and outcome, with no
separate execution ID. Native launch and headless respawn use that private state;
harness input, shutdown and pending operations retain the exact store and
attachment selected at launch. Claude respawn gets a new attachment through the
runtime without changing those earlier snapshots.

The runtime holds a `SessionRecorder` under the same lock. The recorder publishes
input in [`session_record/capture.rs`](../session_record/capture.rs) and writes
history and receipts in [`session_record.rs`](../session_record.rs); it cannot
launch or stop a provider. Provider identity in memory advances before best-effort
history writes, so a recording failure cannot change which account or thread a
retry continues. Later observations retry unrecorded identity without duplicating
successful writes.

The runtime retains the store that admitted its attachment for launch, retry and
cleanup. Completion settles the attachment before writing the receipt; final
destruction also settles unfinished runtime state, including after a recording
panic. A failed receipt write remains a headless caller error. Retrying that
write can record the same outcome but cannot reopen the runtime or change its
result.

Native exit handles attachment cleanup separately and adopts a plain
`SessionRecorder` from `CaptureHandoff` to finish recording. It does not reconstruct
live runtime state. Recording failures there are logged; the provider's exit
result stands. See [execution](../../../../docs/dev/execution.md#runtime-and-recording)
for the ownership model.

## lf never breaks the conversation

lf is a session manager, context adder and hook interjector on top of a harness,
never a middleman that can get in the way (Jack Heart, 2026-11-05). It is never in
the byte path between a person and their provider except as a pass-through that
keeps passing when lf's own state is unavailable: a store that is locked, refused
by its reader floor, missing a column or gone; a hook that fails; a lock that
cannot be read.

Every lf invocation at a turn or tool boundary is best-effort. A bare headless
run ([which launches are managed](../server/README.md#turn-and-tool-boundaries))
has no such boundaries: it runs `__context-block` only at session start and
after a compaction, to print its context block, and asks no server. Otherwise
`__context-block` from Claude hooks, the OpenCode plugin and the Pi extension opens no store: it
sends one `session.boundary` request to the Machine server at `machine.sock`
with a two-second deadline, and the server records the turn, acknowledges the
native input, answers pending steers (delivered once the hook's next request
says it printed them) and, after a turn ends, refreshes planning
and starts a pending Session move ([Turn and tool boundaries](../server/README.md#turn-and-tool-boundaries)).
When the server does not answer, the hook prints the context block without
steers, exits 0 and records the gap once per native conversation in
`logs/lf.log`. The turn or tool completes without it. A recording gap is one
concept, `store::RecordingGap`: one log line per changed failure and one when
lf's state answers again, whether the gap lives in an owner's or observer's
memory or, for hooks that are a fresh process at every boundary, in a
`.store-gap` marker beside the delivery file. The next hook that reaches
the server sends the marker's gap with its request; the server writes it into
the Session's history as one `store-gap` observation (first and last miss) and
the hook clears the marker, so a turn lf never saw reads as unknown rather than
as still running. Work that can outlast a hook's timeout runs beside it: Claude's
`StopFailure` hook starts `--moment record-limit` in its own OS session and exits,
because a kill between a token reply and the profile write would lose a rotated
grant. The Pi extension and OpenCode plugin behave like Claude's
hooks: an lf they cannot run goes to their own log (`logs/pi-extension.log`,
OpenCode's stderr beside the capture), never to the conversation. The PTY
owner delivers typed input even when it cannot record it, and holds the Session
in memory instead. Quitting returns the provider's own exit within a second; exit
records finish in the background or are lost. Nothing about lf's own state ends a
conversation either: a removed, rebuilt or superseded lf executable leaves its
owner serving until the person quits, resumes elsewhere or an operator rolls over.

Codex (`codex_connection.rs`) has no relay: the Codex TUI connects straight to
its private app-server, and lf's observer subscribes beside it to record turns
and deliver Task updates. A store the observer cannot open is a recording gap it
retries; history fills it on the next launch.

## Task update observation

Keep several Task conversations open while a Graph finishes and a teammate adds
a comment. Each conversation receives its relevant changes through the provider's
update path; completed history becomes a baseline, not a report to repeat.

Task update observers read saved comments without resolving planning settings
or delivery badges. Thread views still compute those badges.
Idle polls check the `task_updates` store revision. Its triggers follow Task
membership, Graph/step facts, comments and delivery receipts. Transcript, usage,
availability and ordinary tool-process writes leave it unchanged. Existing Work
revision domains still serve their own readers. The observation counter is shared:
another Task's reportable change can still rebuild history across observers.
Live Graphs and updates awaiting delivery still use the polling clock, so an
unrecorded process exit or a busy provider does not strand an update. Failed reads
remain retryable without another write. Coalescing lets each run settle
independently; one run's churn must not delay another run's report.

### Regression acceptance

Run `cargo test -p loopflow --lib harness::task_updates::tests::` for observation
and delivery behavior. Also include `config::files::tests::`,
`store::sqlite::task_comments::tests::` and the Codex side-channel delivery check
`harness::codex_connection::tests::task_update_wakes_an_idle_conversation_from_the_side`.
Revision upgrades have `store::migrations::tests::task_update_revision_upgrade_preserves_existing_counters`.
The `performance::testing` SQLite trace counters measure statements, rows and VM
steps without a wall-clock gate. `performance_tests::task_wait_reuses_its_store_and_builds_status_only_on_return`
checks the CLI wait loop; `ops::task::tests::task_wait_condition_releases_terminal_tasks`
checks completion, abandonment and missing Tasks.
Use the environment isolation in [verification commands](../../../../scripts/testing.md).

| Scenario | Required check |
| --- | --- |
| Settled Task, expired polling clock | After baselining, each unchanged poll reads only revision counters; no history rebuild or new receipt. |
| Unrelated Session writes and usage updates | Bound queries, returned rows and rebuilds for an idle Task while other work writes; usage-only changes do not rebuild history. Preserve relevant-update delivery. A quiet-store check alone is insufficient. |
| Growing completed history and many observers | Vary history size and observer count independently; unchanged polling cost stays independent of history size and grows at most linearly with observers. Include mixed-work intervals. |
| Repeated settings and comment reads | Count Git children as well as queries. Settings remain fresh without tracking/ignore diagnostics on reads; tracked-file writes still fail. Comment content requires no planning presentation lookup. Distinguish worktree path resolution from protection diagnostics. |
| Delivery deferred or reader unavailable | Retry without another store write; retain receipt-based deduplication across concurrent observers and reconnects. Return to revision-only polling after delivery settles. |
| Live Graph or coalescing candidate | Detect an unrecorded exit and deliver settled changes despite unrelated activity. |

Use deterministic query, row, subprocess and rebuild budgets, with fixture setup
outside the measured interval. Advance test clocks instead of sleeping. Keep
instrumentation in tests; do not add production interfaces solely to count work.
Elapsed times can aid diagnosis, but tight wall-clock thresholds do not belong in
CI. Installed CPU and delivery latency need separate measurements that identify
the build, scheduling policy, observer count and concurrent workload; these tests
do not establish near-zero live CPU.
