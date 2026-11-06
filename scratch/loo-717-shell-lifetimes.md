# LOO-717 — Shell lifetime and Task Session viewers

Jack Heart selected option 1 on November 6, 2026: carry the existing viewer
protocol across SSH and remove the additional remote terminal relay. Implementation
and review publication are authorized; shipping, installation and live-store
migration are not. The existing feature graph returns to demo review after this
revision. The prior repair at `a1067c99cc` is the starting point, not the final
transport design.

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
| Complete and leave | One pinned retirement | Task completion/checkout/branch; existing executor |
| Cleanup becomes eligible | Owned execution and Git resources | Work checks, Desktop shell settlement, Git safeguards |

The owner's existing Process stores `viewer_hold` (`viewed` / `absent`) and
`viewer_hold_at`. No host identity is stored. Connected viewing is refreshed every
ten seconds; a disappeared/stalled owner's last observation ages after thirty
seconds. A clean leave or connection loss dates absence immediately. The stream's
sixty-second replay grace does not add a retention window. Ending an already
unviewed owner preserves its absence timestamp instead of restarting cleanup.
A fresh unrelated viewer never erases uncertainty into immediate deletion:
connected viewing holds, then its later departure starts the normal absence window.

The retirement executor also observes while a completed Task is viewed, so owner
crash needs no new app launch to restart cleanup. Dated absence supplies the
existing deadline; new viewing cancels queued retirement and shell closes.
Unchanged observation must not churn store revisions. Final deletion rechecks
retention and pinned completion/checkout identity.

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

## Delete — do not maintain

- `transport/remote.rs`'s outer client/server heartbeat protocol and PTY-owning
  relay; its dedicated tests are replaced with coverage on the shared viewer path.
- The hidden `SessionCommand::Remote` wrapper contract, its dispatch and SSH argv
  wrapping. Remove the redundant path rather than renaming it or retaining a fallback.
- Duplicate terminal raw-mode/resize/input/output machinery unique to that wrapper.
- The old unretained-shell portion of `machine_commands::remote_companions_*`
  and its shell stub. Real shell coverage lives in `transport/ssh_shells.rs`;
  the surviving file-observer test keeps its checkout and Machine checks.
- `retirement_requires_job_settlement_after_owner_death_and_retries` duplicates
  the final-work/settlement case; its retired-phase and repeat-call assertions
  now live in `retirement_waits_for_final_work_drafts_and_shell_job_settlement`.
- Independent Desktop-shell viewer holds in Task retirement.
- Indefinite `ViewerHold::Unknown`, aggregate `lost_remote`, and SSH-vs-local
  hold classification. Per-connection transport loss feeds dated absence.
- Documentation that any fresh attachment proves a lost viewer exited, that
  remote Quit retains forever, or that Desktop shell views retain checkouts.
- `scratch/explore.html`'s superseded host-occurrence simulation and
  `scratch/concept-review.html`'s prior transport/proof summary. Both remain at
  `a1067c99cc:scratch/`; the current contract and demo record replace them.
- The already removed host occurrence/presence/settlement machinery stays removed.

These runtime, fixture and documentation targets are removed, including the
obsolete `ssh` attachment field in the viewer fixture. Product memory remains
read-only here; its pending correction is in `questions.md`. The surviving
attachment accepts SSH pipes, and only the execution owner evaluates heartbeats.
SSH routing executes its transport argv
directly and adds the local viewer prefix only when returning a prepared command.
Preview handoff follows attachment state; the bridge polls open endpoints and
pending writes after enforcing its queue bound.

Keep: Desktop shell receipts, create-once/attach/Close/Undo, ordinary job settlement,
current Session/process/provider identities, input fencing, requested-work/draft
protections and the Task's one unreleased migration, edited in place.

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

November 6 focused checks: `cargo test -p loopflow --test transport_tests viewer_stream_` passed (4); `cargo test -p loopflow --lib transport::viewer_tests::` passed (7); `cargo test -p loopflow --lib ops::task::retirement::tests::` passed (5, after consolidation); separately filtered real SSH prepared-viewer and shell-failure tests passed (2, `-- --ignored`); `cargo test -p loopflow --test integration machine_commands::remote_file_` passed (2) and the separately filtered `remote_session_open_returns_an_owner_pinned_terminal_command` passed (1). These cover expiry under continuous output, passive-view retention, backpressure, delayed attachment, same-PID reattachment, preview handoff, terminal sizing/restoration, final Session JSON, retained ended-shell history, SSH failure and retirement settlement. Gate owns the broader checks above.

Native demo is still owed: local and real remote Desktop shell reconnect/Close,
orderly Quit delivery, Rex end-to-end Session viewing, lost connection and return,
and no unintended process/PTY/socket survivors. The private demo roots and input
contract are in `loo-717-demo.md`; no private demo provider/app was launched.
Headless framed-transport tests do not establish a physical remote-Machine or
native-host demonstration. Publication, merge and Task completion remain separate.
