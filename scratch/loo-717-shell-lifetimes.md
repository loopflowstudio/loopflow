# LOO-717 — Shell lifetime and Task Session viewers

## Accepted revision — Desktop owns shell persistence

Jack Heart requested on November 6: “yeah, lets remove that API and put it in
the desktop.” This supersedes the general `lf task shell` surface described in
the implementation history below. Shell persistence is a Desktop feature;
Task Session viewing independently owns checkout retention.

Remove `lf task shell`, its Task command dispatch/routing, documentation and
callers. Do not keep a compatibility alias or merely hide the old Task command.
Move shell create/attach/reconnect/Close policy, execution receipts and startup
handling behind a Desktop-owned interface and implementation. Desktop's Swift
workspace consumes that interface. Reuse LF's existing transport, Machine routing,
PTY ownership and process evidence rather than introducing another transport,
server or shell manager. A destination-side runtime entry point may live in the
cross-platform binary for remote Linux execution; it must belong to Desktop,
work without a running GUI, and not route through macOS-only Desktop UI control.
The implementation should choose the smallest existing Desktop boundary that
meets this requirement, not a new general-purpose public shell API.

Preserve the accepted experience: Desktop shell state and jobs survive hide,
switch, quit and network loss; reconnect attaches to the same execution; explicit
Close ends its ordinary jobs; Undo restores history without restarting execution;
new execution is explicit; creation scripts and initial argv never replay.
Keep Desktop's supplied Ghostty integration and no local fallback on remote error.
Rex continues owning its ordinary shells with no LF wrapper or inventory.

LF retains Task Session viewer evidence, the existing scheduled retirement queue,
and checkout/branch cleanup. Shell attachments add no checkout hold. Eligible
cleanup must still settle Desktop-owned shell executions through the narrow
Desktop lifecycle boundary before removing their checkout, while never closing
Rex shells. Preserve cancellation on returning Session viewing, explicit Close,
active work/draft protections, deadlines and retry behavior. Avoid duplicating
retirement policy in Desktop or requiring the app to be open for remote cleanup.

Delete replaced functions/modules and update all CLI, SSH, Swift, tests and docs
together. If receipt/schema names change, edit this Task's unreleased migration
in place; no live-store migration or installation is authorized. Validate the
removed Task command, Desktop's actual local and loopback-SSH launch/reattach/Close
path, same PID/state, no startup replay, quit/disconnect recovery and Session-only
retirement including between-sweep cancellation. Run relevant formatting, Clippy
and affected checks, sync and publish the existing PR, then stop at feature/demo.
Earlier checks do not validate this new ownership boundary. No shipping approval.

### Desktop ownership cut — November 6

Delete — do not maintain: `TaskCommand::Shell`, its selector/dispatch/SSH cases,
`ops::task::workspace::{task_shell,list_shells,ShellBootstrap,shell_command}`,
`ops/task/workspace-shell.sh`, `store/sqlite/task_shells.rs`, the `TaskShell`
export and Task-shell command examples/list-only assertion. Transport's inline
Close policy and `shell_holds_work` input policy move into Desktop too. File observation stays
in Task workspace. Lifecycle, no-replay, history and cleanup tests move to the
Desktop path. Rename the unreleased receipt table to `desktop_shells` in this
Task's existing migration; no compatibility table or second migration.

Implemented boundary: hidden `desktop shell` runtime in the existing command tree,
backed by `desktop::shells` for create/attach/Close, startup and receipt storage.
It dispatches before repository/UI binding, declares that it needs no Desktop
app and uses resident Machine routing plus the existing viewer stream. Swift
supplies its bundled startup resources. Shared transport delegates shell Close
judgment to Desktop; scheduled Task retirement calls Desktop settlement and
cancellation functions. No new service, protocol or per-Task worker.

The deletion cut is committed at `e8385db949`. One private Desktop runtime and one receipt table
remain; file observation stays under Task. Required behavior tests now use that
runtime. No migration, installation or schedule registration touched the live
Machine.

The cleanup reader and store now share `ViewerHold` instead of a reason string
and independent absence timestamp; viewing and absence cannot contradict each
other at this API. Receipt reads carry only checkout and owner, with cancellation
tests reading exact Close intent directly. Desktop startup parsing is private;
its wire fixture no longer requires a public Rust bootstrap type. The leftover
public Task workspace module and process-start timestamp accessor are removed.
Swift derives launch and Close argv from the shell tab itself and shares its
presentation-release path. No deletion target remains.

Focused checks (inherited `LF_*` cleared; disposable stores and built CLI):

- `cargo test -p loopflow --lib desktop_shell` — two passed (removed Task API, GUI-independent runtime, create once).
- `cargo build -p loopflow --bin lf`; `cargo test -p loopflow --test transport_tests shell` — build and four local checks passed, including twenty lifecycle cycles and between-sweep cancellation; three SSH checks explicitly ignored on macOS. Log: `/tmp/loo717-desktop-local.log`.
- `cargo test -p loopflow --test transport_tests ssh_shells::ssh_shell_ -- --ignored` — two passed, including twenty real SSH cycles, PID/state retention, no replay, Close/settlement and failure behavior. Log: `/tmp/loo717-desktop-ssh.log`.
- Focused `--lib` checks `task_workspace_upgrade_preserves_checkout_and_adds_retry_queue`, `returning_viewers_revoke_closes_and_retry_without_a_sweep`, `automatic_shell_close_rechecks_task_viewing_before_consuming_stale_intent` — all three passed. Log: `/tmp/loo717-desktop-retirement.log`.
- `uv run python scripts/check_migrations.py` — passed; one unreleased migration, edited in place.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'remoteOpeningKeepsLocalPlanningAndRetainedDrafts|GhosttyShellBlockTests'` — app/FFI built; seven headless tests passed, including five remote-location cases. Log: `/tmp/loo717-desktop-swift.log`.
- `cargo fmt --all -- --check`; `git diff --check`; `lf context` — passed; scratch fits both limits.
- `cargo test -p loopflow --lib retirement` — 16 passed after the `ViewerHold` reduction; `/tmp/loo717-compress-retirement.log`.
- `cargo nextest run -p loopflow --lib -E 'test(automatic_shell_close_) | test(desktop_shell_)'` — three passed; `/tmp/loo717-compress-shell-policy.log`.
- `cargo nextest run -p loopflow --test transport_tests -E 'test(returning_task_viewer_cancels_shell_close_without_another_sweep) | test(desktop_shell_reconnects_without_replay_and_close_settles_its_jobs)'` — CLI built and both passed, including generic and integrated shell variants; `/tmp/loo717-compress-transport.log`.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'remoteOpeningKeepsLocalPlanningAndRetainedDrafts|departureEndsTerminalViewers|companionSwitchRetainsViews|undoCloseReturnsTheClosedShellTab'` — app/FFI built, four headless tests passed (remote opening includes five cases); `/tmp/loo717-compress-swift.log`.

The Rust compact-unwind linker warning and existing Swift deployment/deprecation
warnings remain. These checks do not demonstrate native hosts or a physical peer.

Current verification and publication still owed are listed under **Remaining
work** below. These focused results cover the Desktop ownership cut; older
publication checks cover the preceding implementation only.

## Accepted decisions and implementation history

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

Desktop shell creation accepts ordinary launch inputs and optional startup
resources; shared transport selects no terminal integration. Swift supplies its
existing Ghostty integration through the private Desktop launch boundary.
Machine routing carries the inputs/resources to the recorded Machine rather
than forwarding a caller-local filesystem path. The later Desktop ownership
revision removes the general Task launcher without changing these semantics.

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

Desktop owns its integration scripts; its runtime selects no implicit Ghostty
bootstrap. Creation without integration, Desktop command/cwd integration and local/loopback-SSH
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
| Open a shell | Desktop shell execution receipt | Desktop create/attach/Close runtime; no checkout hold |
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
viewer or retirement implementation. LOO-498 and LOO-672 remain separately owned;
their native behavior is not established by this branch's transport tests.

## Concept review — November 6

**Current outcome at `57e0eb180`: nothing further to change; no open product
choice.** Source review follows New Shell through Desktop receipts, local/SSH
attachment, lost acknowledgement, Close/Undo and scheduled retirement. The
accepted model needs no additional API or lifecycle: Desktop owns shell policy,
Session viewers own retention, and the shared transport carries both locally and
across SSH. Returning viewing revokes automatic Close; explicit Close and already
settled execution remain distinct. The repaired metadata matches this model.
Usage docs and skill guidance need no further revision. Next proof remains the
affected publication checks and native/physical-remote demo listed below; this
review ran no execution suite and makes no new runtime claim.

**Earlier refinement, now implemented.** At `e8385db949`,
`desktop::result_schema` classified the private runtime as a GUI action, exposed
by exact hidden-command inspection. The registry test made the same assumption.
Lifecycle source inspection found no further mismatch.

Delete — do not maintain: the runtime's GUI result-schema classification and
its inclusion in GUI action-registry validation. Keep the hidden runtime and
dispatch; add no stream DTO, GUI action or public shell API.

The repair committed at `57e0eb180` returns no runtime result schema, limits
registry validation to GUI verbs and documents terminal bytes/Close status in
`desktop/README.md`.
Regression coverage checks exact JSON inspection and exclusion from public
listings alongside existing ordinary-action schemas. Linux/no-app execution,
no-replay reconnect and between-sweep cancellation paths/tests are unchanged.

Compression names the receipt lookup `shell_close_request`: it returns Close
intent, not an owner. Retirement records stay crate-private; they are internal
cleanup state, not another public API. Their reader carries only fields cleanup
consumes; diagnostic reasons and failure counts remain in SQLite and tests read
them there. Clippy exposed those unused fields after narrowing visibility.
The decision notes now link to the design
instead of repeating its implementation and review history. No deletion target remains.

Review reproductions (before repair; inherited `LF_*` cleared, stdin closed):

- `cargo test -p loopflow --lib every_verb_names_a_registry_action` — failed at `desktop.rs:3686`: `shell is not a registry action`; existing macOS compact-unwind linker warning.
- `target/debug/lf help desktop shell --json` — preceding CLI build reproduced `DesktopVerbResult` requiring `window`, `pane`, `action`.

Repair checks (inherited `LF_*` cleared, stdin closed):

- `cargo test -p loopflow --lib every_gui_verb_names_a_registry_action` — one passed; existing compact-unwind warning remains.
- `cargo test -p loopflow --test integration cli_discovery::structured_help_describes_` — two passed, including the built CLI's null runtime schema, public-list exclusion and ordinary GUI schemas.
- `cargo nextest run -p loopflow --lib --test integration -E 'test(desktop_shell_) | test(every_gui_verb_names_a_registry_action) | test(retirement) | test(automatic_shell_close_) | test(cli_discovery::structured_help_describes_)'` — 23 passed after compression; nextest marked the GUI registry check leaky. Log: `/tmp/loo717-compress-metadata.log`.
- `cargo nextest run -p loopflow --lib -E 'test(every_gui_verb_names_a_registry_action)'` — isolated rerun passed without the leak warning; no checkout test processes remained. Log: `/tmp/loo717-compress-registry.log`.
- `cargo clippy --all-targets -- -D warnings` — passed after removing unused read-model fields; diagnostic values remain stored and tested. Log: `/tmp/loo717-compress-clippy.log`.
- `cargo fmt --all -- --check`; `git diff --check`; `lf context` — passed; memory and scratch fit both limits. Broader suites remain with gate; native proof remains at demo.

The `6d42ec8ce` review predates this ownership cut.

The preceding design refinement required returning viewing to revoke queued
cleanup independently of the sweep cadence. It follows Jack Heart's accepted
“return cancels cleanup” contract; it changes neither retention nor timing policy.
The OS scheduler decides when to attempt cleanup. A persisted `closing` phase
cannot authorize a later effect after a Task Session viewer has returned.

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
remains. These results cover the pre-Desktop-cut repair. The current ownership
and cancellation regressions above supply the later focused verification;
broader checks for that cut remain with publication.

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

`desktop::shells` now owns the optional bootstrap script and relative UTF-8
resource contents; `ops/task/workspace.rs` retains file observation only. Swift
packages its maintained scripts from GhosttyResources; the destination materializes
a content-addressed resource directory. Only creation evaluates it. Desktop's
launch script keeps terminal identity, login shell and literal initial argv.

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

November 6 reconciliation at `57e0eb180` confirms the concept-review repair is
committed: exact inspection returns no shell result schema, public listings omit
the runtime, and GUI registry validation covers GUI verbs. Ordinary GUI schemas
retain their regression coverage. The README matches terminal bytes/Close status.
The receipt lookup rename and narrower retirement read model do not change
GUI-independent dispatch, no-replay reconnect or between-sweep cancellation.
No further bounded code mismatch was found. The compression logs listed under
**Concept review** confirm 23 selected checks passed, the registry check's clean
isolated rerun and passing Clippy. No execution suite was rerun for this prose
reconciliation; the earlier local/SSH and Swift checks remain scoped to the
ownership cut.

The local `origin/main` remains `c8d7a7f4a8`; this pass did not fetch upstream.
Product memory includes the runtime/GUI metadata distinction alongside Desktop
ownership, scheduled cleanup and one viewer protocol. Filesystem inspection found
no Product child memory directories. The earlier `lf task status LOO-717 --json`
attempt returned no result before it was stopped; this pass made no new Work
service query or Task/PR state claim. The supplied feature/refresh context defines
this reconciliation.

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

- Review publication: update PR #1811 for the Desktop shell ownership
  revision and committed metadata repair after affected suites. Metadata
  regressions and Clippy passed at `57e0eb180`. The gate selection
  includes `desktop::shells::` and
  `desktop_shell_runtime_needs_no_gui_and_task_shell_is_removed` in place of the
  removed `store::sqlite::task_shells::` filter, plus the Desktop command and
  `cli_discovery::` metadata tests. Sync/publication belong to the
  existing Graph; feature stops at interactive demo. The historical walkthrough
  at `a1067c99cc:scratch/pr-review.html` predates the November 6 replacements.
- Demo: local and real remote Desktop reconnect/Close, orderly Quit delivery,
  Rex end-to-end Session viewing, lost connection/return and survivor inventory
  for processes, PTYs and sockets. Exercise fifteen-minute eligibility plus the
  scheduled cadence, keeping accelerated fixtures distinct from wall-clock proof.
  Rebuild the old private demo app before using it; roots and input contract are
  in `loo-717-demo.md`. No private demo app/provider has launched.

No product choice is open. Native and physical remote-Machine acceptance remain
unproved. Publication, merge and Task completion are separate; this work has no
shipping, installation or live-store migration authorization.
