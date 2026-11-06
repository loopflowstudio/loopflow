# LOO-717 — Shell lifetime and Task Session viewers

Jack Heart selected option 1 on November 6, 2026: carry the existing viewer
protocol across SSH and remove the additional remote terminal relay. Implementation
and review publication are authorized; shipping, installation and live-store
migration are not. The existing feature graph returns to demo review after this
revision. The replacement is implemented at `f86a9a3e6`, integrated with main at
`d5cab9ba60`. The earlier repair at `a1067c99cc` is historical. Jack subsequently
accepted the scheduled cleanup and generic shell-launch revisions below; those
are implemented at `d14b32ada0`, with focused verification complete and the same
review/publication boundary. Concept review found that viewer return must revoke
queued shell closure before the next sweep; the repair below is committed at
`6d42ec8ce`. Publication checks and regression repairs are recorded below at
`3c57ec3bf`; native demonstration follows publication. No new product decision is needed.

## Accepted follow-up — scheduled cleanup and shell launch

Jack Heart accepted this direction on November 6 after questioning the cost of
one retirement process per Task and Ghostty coupling in the Task shell bootstrap.

### One scheduled sweep per Machine

Use the existing SQLite retirement rows as the durable cleanup queue. A single
OS-scheduled command per Machine runs once a minute, reconciles pending retirements,
attempts eligible cleanup and exits. There is no sleeping per-Task worker or
resident replacement daemon. Reuse the launchd/cron scheduling approach already
used by installation, keeping cleanup's job and purpose distinct from updating LF.
Keep the installed binary and Machine data directory pinned in the job. Disposable
homes/tests must never replace the account's real schedule.

The fifteen-minute absence deadline remains authoritative; a minute cadence means
eligible cleanup normally begins in 15–16 minutes while the Machine is running.
After sleep/reboot, the next scheduled invocation evaluates overdue rows from
wall-clock timestamps. Slow job settlement or protected work may delay deletion.
Completion records pending cleanup; viewer release records dated absence. Neither
starts a dedicated retirement process. The sweep must also reconcile retained
rows without a due time so a crashed viewing owner cannot retain a checkout forever.

Each pass rechecks completion identity, viewer evidence, work/drafts/requests,
shell settlement and existing Git safeguards. Return cancels queued cleanup;
reopening or changing the checkout/completion revokes the old intent. If owners
are still closing, persist that phase and finish on a later sweep, without waiting
in a two-second loop. Keep runs serialized and effects repeatable; one failed Task
must not prevent processing other pending Tasks. Persist failure reasons and retry
eligible failures with backoff, separately from the absence deadline. Interrupted
passes remain recoverable on the next scheduled invocation. Do not repeatedly
query providers or scan all repositories when the local queue has no work.

This explicitly resolves the previously deferred cleanup-worker retry finding.
Implement schedule provisioning in the normal Machine installation/setup path;
validate it with isolated fixtures. This development run does not install/register
a real job on Jack's Machine or migrate its live store. Report missing scheduler
support rather than silently restoring the removed per-Task process.

### Generic launch, Desktop-owned shell integration

Separate shell execution/lifetime from terminal-host integration. The shared Task
shell launcher accepts ordinary launch inputs and does not unconditionally select
Ghostty, materialize its scripts, or reach into the Swift app's resource tree.
Desktop explicitly supplies/selects its existing Ghostty shell integration through
the launch boundary; transport carries the needed launch inputs/resources to the
recorded Machine rather than forwarding a caller-local filesystem path.

Reuse the maintained integration scripts and their prompt/command/cwd markers.
Do not write a competing shell integration or add a generic plugin/host registry.
The generic path must work without Ghostty-specific environment or startup changes.
Preserve personal login startup, literal initial argv, selected LF availability,
terminal identity, command blocks and remote reconnect semantics. Integration is
applied on shell creation, never by replaying startup on reattachment. Rex still
owns its ordinary shells and their integration; LF adds no Rex-shell ownership.

### Delete — do not maintain

Removed: `retirement::start`, its executor loop and executor-only locks;
`lifetime::record_hold` and viewer-release process launching; `transport/remote.rs`
and `SessionCommand::Remote`; Rust's `workspace-bash-env` / `workspace-zshenv`;
Swift's `LoopflowZshBootstrap` and `buildWorkspaceShellCommand`.
The finite sweep returns only success/failure; its former finished-worker boolean
is gone. The concept-review repair also removes the phase-only
`SqliteStore::owner_retiring` judgment; owners and the sweep now share the current
Task viewer judgment. The boolean-only `record_owner_viewers` writer and its
separate recorder/retry state are removed too: one dated observation updates
`viewed`, the hold and cancellation together. Tests inspect checkout and queue
state. No deletion target remains.

Verification obligations for the surviving paths:

The sweep retains cleanup intent, effect serialization, final rechecks and
explicit cleanup commands. Scheduled-pass tests cover many Tasks sharing one
schedule, prompt empty-queue exit, returning viewers and active work, expired/dead
viewers, interrupted/partial deletion, retry/backoff and failure isolation.

Desktop owns its integration scripts; shared shell creation selects no Ghostty
bootstrap. Generic creation, Desktop command/cwd integration and local/loopback-SSH
reattachment must preserve PID/state without reapplying integration. Keep one
unreleased migration, the shared viewer protocol and existing cleanup safeguards.
Sync, focused verification, publication and demo review remain the delivery path.

## Accepted experience

- Desktop shells execute on the Task's recorded Machine and checkout. Hide,
  switching Work and app exit preserve execution until eligible cleanup. Reconnect
  reuses that execution; New Shell is deliberate and never replays old commands.
  Remote failure never produces a local successor prompt.
- Close ends a Desktop shell and its ordinary jobs; Undo restores identity/history,
  not execution. LF cleans only Desktop-owned shells. Rex owns its shells, layouts
  and restoration; an ordinary Rex shell may survive deletion of its cwd.
- **Only Task Session viewers retain the checkout.** Hidden, unfocused and passive
  viewers count; input control is separate. Any remaining connected viewer holds.
  Jack clarified that Desktop shells already accompany Task Sessions, so they add
  no independent hold. Cwd, native tab titles and saved screens grant no hold.
- A viewer's confirmed exit releases it. A surviving native Rex tab after the LF
  viewer exits is not another hold. Provider and viewer exit are separate events.
- **Orderly Desktop Quit explicitly releases its viewers**, like window close.
  Failure to deliver leave follows the disconnect policy rather than claiming
  successful release.
- **A disconnected viewer uses the same fifteen-minute absence window.** Jack
  accepted this with “sounds good.” Count from detected loss; do not add another
  fifteen minutes after an uncertainty expiry. Reconnect cancels queued cleanup.
  Expiry releases retention without claiming the client process died. Returning
  after a long outage may find a completed Task's checkout gone; history remains.
- Only completed Tasks acquire checkout cleanup. Eligible idle agents use the
  existing two-minute policy. Requested work, drafts, pending requests and running
  Graphs still protect execution. Work finishing after the deadline adds no new
  fifteen-minute wait. Ordinary shell output is not an agent turn.
- Cleanup discards eligible tracked/untracked uncommitted files. Existing merged
  delivery, committed-ref, primary/persistent-checkout and unrelated-resource
  safeguards remain. PR-less checkouts remain retained by that existing safeguard.
  Automatic restoration of deleted checkouts stays outside scope.
- No external-host/workspace/occurrence persistence: no registry, sidecars,
  creation journal or native-shell settlement handshake. Use Session/process
  evidence. This does not prohibit dated connection evidence on existing Processes.

The accepted bounded-disconnect policy supersedes the earlier indefinite unknown
hold and earlier rejection of expiry. Earlier external-host occurrence/lease
architecture is at `b81df60579:scratch/loo-717-shell-lifetimes.md`; the published
indefinite-viewer implementation is at `500425244c`. Those are history, not current
requirements. The pre-realignment uncommitted review record is preserved in
`/Users/jack/.lf/demos/loo717-500425244c/durable/design-before-repair-realignment.md`.
All current requirements are included here.

## One walkthrough

Complete a Task while viewing its Session in Desktop or Rex: the checkout stays.
Hide the Rex tab: its LF viewer remains connected and still holds. In Desktop,
leaving the Task detaches its views under Desktop's host policy. Close the last
Session viewer: normal absence cleanup starts. Its ordinary shell adds no hold.
LF eventually closes its Desktop-owned shells; Rex's shells remain Rex's.

Lose the remote connection without EOF: LF's remote path detects missing client
heartbeats and releases that connection. The execution owner remains independent.
Reconnect before the cleanup deadline: retain again. Leave it disconnected: a
completed Task can retire after the same fifteen-minute absence window, once
protected work permits. A second connected viewer keeps the checkout throughout.

## Model and implementation

| Action | Existing concept | Owner/API |
| --- | --- | --- |
| View or passively observe a Task Session | Session-associated viewer | Transport Attach/Resume, separate input claim |
| Hide a still-open native view | Same connection | No host focus report |
| Open a shell | Shell execution receipt | task shell create/attach/Close; no checkout hold |
| Leave or lose the last connection | Dated absence | Serving Process viewer evidence |
| Complete and leave | One pinned retirement | Task completion/checkout/branch; scheduled Machine sweep |
| Cleanup becomes eligible | Owned execution and Git resources | Work checks, Desktop shell settlement, Git safeguards |

The owner's existing Process stores `viewer_hold` (`viewed` / `absent`) and
`viewer_hold_at`. No host identity is stored. Connected viewing is refreshed every
ten seconds; a disappeared/stalled owner's last observation ages after thirty
seconds. A clean leave or connection loss dates absence immediately. The stream's
sixty-second replay grace does not add a retention window. Ending an already
unviewed owner preserves its absence timestamp instead of restarting cleanup.
A fresh unrelated viewer never erases uncertainty into immediate deletion:
connected viewing holds, then its later departure starts the normal absence window.

The scheduled sweep reconciles pending cleanup even while a completed Task is
viewed, so owner crash needs no new app launch. Dated absence supplies the existing
deadline; new viewing cancels queued retirement and shell closes. Unchanged
observation must not churn store revisions. Final deletion rechecks retention and
pinned completion/checkout identity. The previously published per-Task executor
is superseded by the accepted follow-up above.

### Accepted transport revision — November 6

Use the existing viewer protocol for local and remote connections. Attach/Resume,
input, output, resize, input ownership and Leave retain their current meanings.
Add heartbeat/liveness to that same connection through its existing `Controlled`
query; expiry is enforced by the serving
owner against the actual viewing endpoint, not by a remote process heartbeating
to another local process on behalf of an unreachable client.

The path is local viewer → SSH byte stream → existing execution owner.
A small stdio-to-socket bridge forwards the existing frames.
It must not allocate another PTY, invent a second framing protocol, synthesize
heartbeats, or own another Session. The existing execution owner keeps its PTY
and survives viewer loss. Heartbeats remain independent of typing/provider output;
retain the ten-second cadence and thirty-second detection bound. Expiry releases
only that viewer's retention and input claim. A second connected viewer still holds.

The shared attach loop reads a socket or command pipes. Prepared remote Session
opening and remote Desktop shell create/attach use that path end to end. Preserve recorded Machine/checkout, authentication routing,
native launch preparation and no-replay behavior. Do not wrap arbitrary interactive
commands in a second terminal merely to support Session viewers. SSH remains the
authenticated transport; no sshd configuration change, new resident server or
external-host registry is needed. An LF viewer running inside a retained Rex
terminal counts while that viewer remains connected to its owner. An independent
external SSH connection cannot inherit a heartbeat guarantee across a network
hop LF does not carry.

Acceptance checks on the replacement: local and real loopback-SSH attach/input/
resize/leave; silent loss without EOF while output continues; healthy idle and
passive viewers; two viewers with one lost; reconnect before retirement; unchanged
execution PID/state after detach/reattach; explicit Close settling owned shell jobs;
initial 24×80 sizing, backpressure and terminal flag restoration. Exercise the
production prepared-launch path, not just a protocol fixture. Preserve the existing
retirement checks and demonstrate the extra relay PTY is gone. Native Desktop/Rex
and physical remote-Machine proof remains separate.

Desktop Quit sends the existing detach key to retained relays and waits up to
two seconds for their exit. Failure to finish within that bound uses network-loss
cleanup. Shell views do not independently veto retirement; requested work nested
inside a shell remains protected by existing work/draft/request evidence.

## Implemented replacement — reconciled November 6

The existing `transport::relay_over` reads the same frames over command pipes;
`transport/stream.rs` bridges destination stdio to the owner socket. SSH Session
preparation returns the local viewer prefix around SSH argv; direct routing runs
that argv without nesting viewers. Saved history arrives as a snapshot before
attachment and grants no input or retention. Only the owner evaluates heartbeats.

Removed: `transport/remote.rs`, `SessionCommand::Remote`, its dispatch/SSH wrapper,
and the duplicate PTY, framing, raw-mode and resize machinery. Shared viewer and
real SSH tests replace its dedicated tests. The shell stub in the remote companion
test is gone; file observers retain their own Machine/checkout checks. Retirement
settlement/repeat-call assertions are consolidated in the final-work test.
Desktop-shell holds, indefinite unknown holds and host-occurrence machinery remain
removed. Superseded HTML simulations and deletion details remain at
`a1067c99cc:scratch/`; `d5cab9ba60:scratch/loo-717-shell-lifetimes.md` records the
replacement's full deletion checklist.

Desktop shell receipts, create-once/attach/Close/Undo, job settlement, Session and
provider identity, input fencing and requested-work/draft protections remain.
The Task still has one unreleased migration. Product memory now reflects the
November 5–6 decisions; no child Product memories exist in this checkout.

The current base `c8d7a7f4a8` changes builtins/Graph guidance, not the shared
viewer or retirement implementation. Product memory includes the November 6
scheduled-sweep and generic-launch decisions. LOO-498 and LOO-672 remain separately owned; their native
behavior is not established by this branch's transport tests.

## Concept review — November 6, scheduled follow-up

**Current outcome at `6d42ec8ce`: no further design change and no open product
choice.** Source review follows completion/leave through queued closure, viewer
return between sweeps, interrupted cleanup and reattachment. The repair below
implements the required cancellation; the finite queue, shared SSH viewer stream
and Desktop-supplied startup resources need no additional product concept.
Existing usage docs describe Close/Undo and recovery consistently. Gate and native
demonstration remain the next proof; this review reran no execution checks.

The preceding design refinement required returning viewing to revoke queued
cleanup independently of the sweep cadence. It follows Jack Heart's accepted
“return cancels cleanup” contract; it changes neither retention nor timing policy.
The OS scheduler decides when to attempt cleanup. A persisted `closing` phase
cannot authorize a later effect after a Task Session viewer has returned.

Normal path: complete the Task, leave its last Session viewer, and let one sweep
queue Desktop shell closure after the absence deadline. Owners settle their jobs;
a later pass removes eligible Git resources. The retirement row carries progress
and retry state across interrupted passes. Desktop supplies startup resources only
when creating a shell; local/SSH attachment reuses execution without replay.

Counterexample at `d14b32ada0`: a sweep writes `closing` and
`close_requested=2`, then exits while an owner has not yet consumed that request.
A Task Session viewer returns and its owner records `Viewed`. Both
`record_owner_viewers` and `record_owner_viewer_hold` update only the Process.
Before another sweep, the separate shell owner's `Ending::confirm` sees the old
close request and `owner_retiring` sees `closing`, so it can terminate the shell
and its jobs. Neither checks the Task's renewed viewing. The next sweep retains
the checkout but cannot restore the lost shell state. A retry backoff does not
prevent owners from consuming that pending request.

Relevant sources: `store/sqlite/task_retirement.rs`, `store/sqlite/sessions.rs`,
`transport/lifetime.rs`, and `ops/task/retirement.rs`. The earlier return tests called
reconciliation/`advance` before checking cancellation; they did not exercise the
owner consuming a close between passes. This is a source-derived counterexample,
not a newly run process demonstration.

Repair contract (implemented at `6d42ec8ce`):

- Record renewed Session viewing and revoke that Task's pending retirement and
  retirement-generated shell closes together. Use current Session-to-Task
  membership, including passive viewers; an ordinary shell viewer adds no hold.
  Preserve explicit Close and already-settled effects. A brief return/leave must
  still restart absence and clear retry state without waiting for a sweep.
- Before an owner consumes automatic closure, judge the current Task viewing and
  current retirement intent, rather than treating `closing` as unconditional
  permission. Reuse the existing records and shared judgment; add no host registry,
  per-Task worker, second cleanup policy or blocking lock in the conversation path.
  Unreadable cleanup state retains execution; store failures still cannot stop
  conversation bytes. Keep the sweep's final completion/placement and Git checks.
- Keep the accepted recovery limits: cancellation cannot resurrect a shell whose
  closure already completed; history survives and New Shell is deliberate. Losing
  one controlling viewer while another passive viewer remains never starts cleanup.
  Rex shells stay outside LF's closure policy.

Applied usage correction in the workspace section of `swift/README.md`:
“⇧⌘T restores the last closed tab's identity and history. Close ends its shell
and jobs; New Shell starts fresh execution.” The former sentence promised
“its shell still running,” contradicting both the accepted contract and the later
README section. No skill needs a new concept.

Focused regression: queue automatic closure, record a Task Session viewer's return, then
let the shell owner judge it **without another sweep**. Assert the shell and jobs
survive. Cover a passive viewer on another Session, return/leave between passes,
explicit Close remaining effective, and an ordinary shell viewer granting no hold.
The store regressions also cover rollback if cancellation fails and a brief
return/leave coalesced into one dated absence observation. Final owner checks
cover stale intent, recent absence, explicit Close, ordinary shell viewing and
unreadable cleanup state. The real transport regression pauses the shell owner,
queues cleanup, returns a passive viewer on a different Session, then resumes the
owner without any sweep; shell state and a background job must survive. A second
pass leaves again before the owner resumes. Gate and native demo remain below.

The recorder uses one transaction for the Process's `viewed` flag, dated hold
and cleanup cancellation. This replaces separate writes and retry tracks
for the same observation, including on owner exit. Unchanged Process values do
not advance store revisions; a failed transaction retries the whole observation.

Focused verification at `6d42ec8ce:scratch/loo-717-shell-lifetimes.md`, this
section: 21 library and two real transport tests passed after recorder
consolidation, with formatting clean. The initial nullable-`viewed` rollback
assertion was corrected and passed; the macOS compact-unwind linker warning
remains. These results cover the current code and are reused, not rerun for
prose reconciliation. Broader gate owns Clippy and affected suites.

## Scheduled cleanup and generic launch — November 6 implementation

The per-Task detached worker, executor lock, two-second loop and viewer-release
spawn path are removed. `task retire [task]` runs one finite pass over the existing
queue, including held rows. A Machine lock serializes sweeps; existing Task locks
serialize effects with reopen/completion. Pending shell closes continue on later
passes. Errors record their reason and exponential retry deadline (60 seconds,
up to one hour), independently of absence; another Task still runs. Return clears
retry state. The same unreleased migration owns the final queue fields.

Normal installation/adoption provisions a separate `com.loopflow.cleanup` launchd
job or marked cron entry, every minute, with the installed binary and Machine
home pinned. Disposable HOME/LF_HOME never replaces account schedules. No live
schedule, installed app or live-store migration was performed in this run.
Linux renders both distinct jobs into one crontab update, sharing path quoting;
launchd shares plist path escaping. The final sweep recheck reads viewing and
pinned completion/placement together instead of querying the intent twice.

`workspace.rs` no longer selects Ghostty or embeds Swift resources. Its optional
bootstrap input is a script and relative UTF-8 resource contents. Desktop packages
its maintained scripts from GhosttyResources; the destination materializes a
content-addressed resource directory. Only creation evaluates it. Existing
Desktop-specific wrappers moved out of Rust into Desktop resources; the core
keeps terminal identity, login shell and literal initial argv.

The earlier manual retry check is replaced by a scheduled-pass test that records
backoff after partial Git deletion, lets another Task complete, then recovers on a
later pass without replaying the completed remote deletion. Empty queue,
interrupted close, backoff reset, schedule idempotence/isolation and final-schema
upgrade have focused checks. Local and loopback-SSH shell tests carry the real
Desktop scripts and retain generic no-integration coverage. Gate and native demo
remain separate from these focused checks.

Focused verification recorded at `d14b32ada0:scratch/loo-717-shell-lifetimes.md`
passed: 17 initial cleanup/schedule/migration checks; local generic and Desktop
shell variants; twenty real loopback-SSH lifecycle cycles; 66 headless Desktop
checks; empty-queue exit; architecture, formatting and Clippy. Compression then
passed 35 affected library checks and 15 after final query/import edits, plus
Clippy. These checks cleared inherited `LF_*` and used isolated stores. The
initial fixture branch collision and fixture mutex-scope finding were repaired;
the macOS compact-unwind and existing Swift warnings remain. No native-host or
physical remote-Machine demonstration follows from these results.

## Related work

- LOO-672 owns automatic Desktop conversation opening/prefetch and warmth. Its
  Session views supply retention; it does not own another cleanup policy.
- LOO-498 owns native layout creation in Rex/Herdr/cmux, fresh on explicit open.
  Hosts own subsequent arrangement/restoration. LF DB owns Desktop layouts only.
  No external layout persistence or LF closure of Rex shells is reintroduced.
- LOO-709 owns ordinary eligible idle-agent ending. Shell lifetime remains distinct.

`rex-coordination.md` keeps the current boundary, unresolved native proof and
Git references to the superseded handoff and external-host design.

## Checks and remaining proof

November 6 reconciliation checked the committed repair at `6d42ec8ce`: one
transaction records viewing and cancels automatic closes, and owners recheck
Task viewing and retirement intent before consuming automatic closure. The
no-sweep transport regression covers a passive viewer on another Session and
brief return/leave; library checks cover ordinary shells, rollback and unreadable
cleanup state. Explicit Close remains effective. The README's Undo promise is
corrected. No further bounded implementation mismatch was found. The local
`origin/main` remains `c8d7a7f4a8`; no newer upstream change is available in that
reference. Product has no child memory directories in this checkout.

Reconciliation check: `git diff --check`; `lf context` — passed; memory and scratch fit both size targets.

The earlier publication record at
`d14b32ada0:scratch/loo-717-shell-lifetimes.md`, **Publication checks**, applies to
`c7f1afa1d8`, before the scheduled-sweep/generic-launch follow-up. All 74 selected
checks passed across an interrupted nextest run and its twelve-case continuation
with the built CLI pinned; three real loopback-SSH tests passed. The first run
alone did not pass: four fixtures queued CLI builds and eight had not started.
Architecture, formatting and Clippy passed. The broader
`scripts/test.py --reuse-passing` gate stopped twice at its 15-second uv-cache inventory preflight,
before product suites. Those broader suites remain unverified, not failed product
assertions. The full commands, counts, warnings and prior transport checks remain
at that reference; they do not validate the follow-up by inheritance.

### Publication checks — November 6, `3c57ec3bf`

`lf sync` merged cleanly against `c8d7a7f4a8`. The affected-suite runner passed
resource preflight this time. Its first aggregate result was **failed**, exposing
two publication fixtures relying on ambient account/agent selection, one old
transport diagnostic assertion, and three Swift assertions expecting the old
shell/surface retention behavior. The four test files were repaired; no production
code changed during publication. Publication fixtures now use their own account,
agent and cwd. Swift checks require disconnected history and release on departure
while retaining Session/layout/draft identity. Focused reruns passed.

- `uv run python scripts/test.py --reuse-passing --rust-filter 'test(transport::) | binary(transport_tests) | test(ops::task::) | test(store::sqlite::task_retirement::) | test(store::sqlite::task_shells::) | test(store::migrations::) | test(lf::commands::install::published::) | test(lf::commands::ssh::) | test(machine_commands::) | test(land_tests::) | binary(lf)'` — architecture and 72 website tests passed; Rust 419/422 passed initially; full headless Swift run reported 687 tests with three stale assertions. Original log: `.lf/tmp/gate/run-89859/`. This aggregate run did not pass.
- Network-isolated nextest boundary/repair selection — 75/77 passed, including startup, journal, Task Graph launch, Git cleanup, revision coverage and the corrected transport diagnostic; the two publication fixtures needed cwd isolation as well. Log: `/tmp/loo717-publish-boundaries.log`.
- Network-isolated nextest `test(final_preparation_preserves_published_copy_only_for_the_same_head) | test(land_generates_copy_when_cached_pr_copy_is_stale)` — both passed after cwd isolation. Log: `/tmp/loo717-publish-copy-repair.log`. Across the original and focused runs, all 496 selected Rust checks passed; there was no fresh all-green aggregate rerun.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'SessionsPolishTests|WorkCLISubscriptionTests'` — seven tests passed, including both lifecycle arguments; app/FFI rebuilt. Log: `/tmp/loo717-publish-swift-repair.log`. Display diagnostics remain skipped.
- `cargo test -p loopflow --test transport_tests ssh_shells:: -- --ignored` with built `LOOPFLOW_TEST_LF` — three passed, including twenty lifecycle cycles. Log: `/tmp/loo717-publish-ssh.log`.
- `uv run python scripts/check_migrations.py`; `uv run python scripts/check_swift_multiplatform_boundaries.py`; `cargo fmt --all`; `cargo clippy --all-targets -- -D warnings`; `git diff --check` — passed.
- `uv run python scripts/test_task_installation.py --repository-upgrade` — disposable Linux release build and populated 0.14.3 upgrade passed: Task/Session identity, history, edits, move and retry. Container removed. Log: `/tmp/loo717-publish-upgrade.log`.

Tests ran with inherited `LF_*` cleared and stdin closed. Native compact-unwind
and existing Swift/deployment-target build warnings remain. No live installation,
scheduler registration, credential change or live-store migration occurred.

Remaining work:

- Review publication: update PR #1811 for the scheduled-sweep/generic-launch
  revision after the required checks. The prior `scratch/pr-review.html`
  walkthrough describes `a1067c99cc` and predates both November 6 replacements.
- Demo: local and real remote Desktop reconnect/Close, orderly Quit delivery,
  Rex end-to-end Session viewing, lost connection/return and survivor inventory
  for processes, PTYs and sockets. Exercise fifteen-minute eligibility plus the
  scheduled cadence, keeping accelerated fixtures distinct from wall-clock proof.
  Rebuild the old private demo app before using it; roots and input contract are
  in `loo-717-demo.md`. No private demo app/provider has launched.

No product choice is open. Native and physical remote-Machine acceptance remain
unproved. Publication, merge and Task completion are separate; this work has no
shipping, installation or live-store migration authorization.
