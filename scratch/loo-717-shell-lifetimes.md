# LOO-717 — Shell lifetime and Task Session viewers

Jack Heart selected option 1 on November 6, 2026: carry the existing viewer
protocol across SSH and remove the additional remote terminal relay. Implementation
and review publication are authorized; shipping, installation and live-store
migration are not. The existing feature graph returns to demo review after this
revision. The replacement is implemented at `f86a9a3e6`, integrated with main at
`d5cab9ba60`. The earlier repair at `a1067c99cc` is historical. Jack subsequently
accepted the scheduled cleanup and generic shell-launch revisions below; those
are implemented in the working tree, with focused verification complete and
the same review/publication boundary.

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
is gone. Tests inspect checkout and queue state. No deletion target remains.

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

The upstream sync at `c8d7a7f4a8` changes builtins/Graph guidance, not the shared
viewer or retirement implementation. The current refresh Graph includes this
memory reconciliation. LOO-498 and LOO-672 remain separately owned; their native
behavior is not established by this branch's transport tests.

## Concept review — November 6

No design change or open product choice. The implemented local/SSH viewer path
fits the accepted interaction: open on the recorded Machine, detach without
ending execution, reconnect to that owner, and use New Shell deliberately after
execution ends. A shell receipt represents execution; a Session viewer represents
checkout retention; the retirement record represents completed Task cleanup.
Each has a separate job. No host registry or second remote terminal is needed.

The decisive counterexample is a lost controlling viewer with a second passive
viewer still connected. Input ownership may change, but checkout retention must
remain. The owner judges each connection before recording aggregate viewing;
the bridge cannot keep a lost endpoint alive with its own heartbeat. On last-viewer
loss, dated absence supplies one cleanup window. Return before deletion cancels
retirement and its queued shell closes, while explicit Close remains explicit.
Return after deletion shows history without reconstructing a checkout or replaying
commands. Ordinary Rex shells remain outside LF's closure policy in every case.

The existing transport and Desktop usage docs describe these actions consistently;
no skill instruction needs a new concept. This review did not resolve cleanup-error
retry; Jack's subsequent scheduled-sweep decision above now includes that repair.
Next proof remains the affected gate and native demo already listed below,
especially orderly Quit delivery and repeated reconnect/Close with survivor
inspection. This was source review; no runtime or native checks were rerun.

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

Focused follow-up verification (November 6; fixture commands cleared inherited
`LF_*`, no live installation or store migration):

- `scripts/check_migrations.py` — passed; one final draft migration.
- `cargo nextest run -p loopflow --lib -E 'test(ops::task::retirement::tests::) | test(store::sqlite::task_retirement::tests::) | test(cleanup_schedule_) | test(launchd_schedule_preserves_frequency) | test(task_workspace_upgrade_)'` — 17 passed. An initial fixture branch collision was corrected before this pass.
- `cargo build -p loopflow --bin lf --jobs 4` — passed.
- `cargo test -p loopflow --test transport_tests desktop_shell_reconnects_without_replay_and_close_settles_its_jobs` — passed, both generic and Desktop bootstrap variants; selected LF, terminal identity, literal argv, retained state and settled jobs.
- `cargo test -p loopflow --test transport_tests ssh_shells::ssh_shell_lifecycles_reconnect_and_settle_processes_ptys_and_sockets -- --ignored --test-threads=1` — passed, twenty real loopback-SSH cycles (111.28 s) carrying Desktop resource bytes, reconnecting and settling processes/PTYs/sockets.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'GhosttyShellBlockTests|WorkDestinationTests'` — app/FFI built; 66 headless tests passed. Existing Swift warnings and the Rust test linker's compact-unwind warning remain unchanged.
- `lf task retire` in a disposable Machine home outside a repository — passed with an empty queue in 0.15 s.
- `scripts/check_architecture.py`, `cargo fmt --all`, `cargo clippy --all-targets --jobs 4 -- -D warnings`, `git diff --check` — passed. Clippy's fixture mutex-scope finding was fixed. Gate owns the broader affected suites and demo owns native Desktop/Rex and physical remote-Machine checks.

Compression verification (November 6; inherited `LF_*` cleared for tests):

- `cargo nextest run -p loopflow --lib --build-jobs 4 --test-threads 4 -E 'test(ops::task::retirement::tests::) | test(store::sqlite::task_retirement::tests::) | test(lf::commands::install::published::tests::) | test(transport::lifetime::tests::)'` — 35 passed; existing macOS compact-unwind linker warning.
- `cargo nextest run -p loopflow --lib --build-jobs 4 --test-threads 4 -E 'test(ops::task::retirement::tests::) | test(store::sqlite::task_retirement::tests::) | test(cleanup_schedule_)'` — 15 passed after the final query/import edits.
- `cargo clippy --all-targets --jobs 4 -- -D warnings` — passed, including FFI. Broader gate and native demo retain their owners below.

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

Prior repair evidence remains at `a1067c99cc:scratch/loo-717-shell-lifetimes.md`.
The shared viewer revision has focused checks below; the prior retirement,
Clippy, Rust FFI and Swift results do not establish this revision's full gate.
Gate owns the affected `transport::`, `transport_tests`, `machine_commands::`,
CLI binary, Session routing and Task workspace/retirement suites, plus architecture,
website and Swift checks selected by `scripts/test.py --list`. macOS loopback SSH
checks run directly with `--ignored`, outside the nested network sandbox.

Inherited November 6 checks (before upstream sync): `cargo test -p loopflow --test transport_tests viewer_stream_` passed (4); `cargo test -p loopflow --lib transport::viewer_tests::` passed (7); `cargo test -p loopflow --lib ops::task::retirement::tests::` passed (5, after consolidation); separately filtered real SSH prepared-viewer and shell-failure tests passed (2, `-- --ignored`); `cargo test -p loopflow --test integration machine_commands::remote_file_` passed (2) and the separately filtered `remote_session_open_returns_an_owner_pinned_terminal_command` passed (1). These cover expiry under continuous output, passive-view retention, backpressure, delayed attachment, same-PID reattachment, preview handoff, terminal sizing/restoration, final Session JSON, retained ended-shell history, SSH failure and retirement settlement. Gate owns the broader checks above.

Native demo is still owed: local and real remote Desktop shell reconnect/Close,
orderly Quit delivery, Rex end-to-end Session viewing, lost connection and return,
and no unintended process/PTY/socket survivors. The private demo roots and input
contract are in `loo-717-demo.md`; no private demo provider/app was launched.
Headless framed-transport tests do not establish a physical remote-Machine or
native-host demonstration. Publication, merge and Task completion remain separate.

Reconciliation check: `git diff --check` — passed; no runtime code changed or tests rerun. Broader gate and native demo remain assigned above.

Publication checks, November 6 (`c7f1afa1d8`, after clean `lf sync`):

- `uv run python scripts/check_architecture.py` and `git diff --check` — passed.
- `scripts/test.py --reuse-passing` with the affected Rust filter — stopped twice at resource preflight, before product suites: uv-cache inventory exceeded its 15-second deadline. Direct `du -sk /Users/jack/.cache/uv` completed in 14.65 seconds; 144 GiB remained free. The retry still timed out. Broader gate, website and Swift checks remain deferred to gate/CI, not passed.
- Focused `cargo nextest run -p loopflow --build-jobs 4 --test-threads 4` — 62 tests passed before four fixtures queued CLI builds because this direct invocation omitted `LOOPFLOW_TEST_LF`. Interrupted those four (no product assertion failed); eight had not started. The run as a whole did not pass.
- `cargo build -p loopflow --bin lf --jobs 4`, then nextest with `LOOPFLOW_TEST_LF=$PWD/target/debug/lf` for the twelve remaining cases — passed (12). Combined with the first run, all 74 selected checks passed: transport library, retirement/store, Session holds, local shell cycles, shared-stream expiry/backpressure, preview restoration and remote launch/file routing. macOS linking reported the large `__eh_frame` compact-unwind warning.
- `LOOPFLOW_TEST_LF=$PWD/target/debug/lf cargo test -p loopflow --test transport_tests ssh_shells:: -- --ignored --test-threads=1` — passed (3, 197.21 seconds), including prepared launch with no extra remote PTY, healthy idle heartbeat, resize/Leave, failure paths, and twenty remote lifecycle cycles with process/PTY/socket settlement. All fixture commands cleared inherited `LF_*`; no installed app, live-store migration or physical remote Machine was used.
- `cargo fmt --all -- --check` and `cargo clippy --all-targets --jobs 4 -- -D warnings` — passed; Clippy also checked `loopflow-ffi`.

The 74-case nextest selection was `test(transport::) | test(viewer_stream_) | test(session_viewers_retain_) | test(desktop_shell_) | test(ops::task::retirement::) | test(store::sqlite::task_retirement::) | test(machine_commands::remote_session_open_returns_an_owner_pinned_terminal_command) | test(machine_commands::remote_file_)`. The twelve-case continuation used `test(snapshot_preview_resume_) | test(viewer_stream_) | test(session_viewers_retain_) | test(desktop_shell_) | test(machine_commands::remote_session_open_returns_an_owner_pinned_terminal_command) | test(machine_commands::remote_file_)` against identical source.
