# LOO-717 — Shell lifetime and Task Session viewers

Jack Heart approved the revised behavior and requested its repair on November 5,
2026. Implementation and review publication are authorized; shipping, installation
and live-store migration are not. Task position remains feature/demo. The direct
repair is validated after the requested pursue failed before implementation
on a Claude Keychain credential-write error. No credentials/accounts were changed.

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

LF's remote route uses framed PTY input/output over SSH. Its local viewer sends
heartbeats every ten seconds, independent of provider output or typing. The
remote relay ends after thirty seconds without a heartbeat, even when TCP has
not delivered EOF; it ends only the command/viewer group it created, not the
independently detached execution owner. Dimensions travel on that connection.
This is a foreground transport, not a resident service or external-host registry.
Prepared remote Session argv and ordinary interactive Machine routing use it.
An independently opened external SSH session still uses its host's connection
loss detection; LF cannot claim its own heartbeat bound for that different path.

Desktop Quit sends the existing detach key to retained relays and waits up to
two seconds for their exit. Failure to finish within that bound uses network-loss
cleanup. Shell views do not independently veto retirement; requested work nested
inside a shell remains protected by existing work/draft/request evidence.

## Delete — do not maintain

- Independent Desktop-shell viewer holds in Task retirement.
- Indefinite `ViewerHold::Unknown`, aggregate `lost_remote`, and SSH-vs-local
  hold classification. Per-connection transport loss feeds dated absence.
- Documentation that any fresh attachment proves a lost viewer exited, that
  remote Quit retains forever, or that Desktop shell views retain checkouts.
- The already removed host occurrence/presence/settlement machinery stays removed.

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

`rex-coordination.md` contains the complete earlier handoff and subsequent
Session-view clarification. Its older retained-native-tab and indefinite-unknown
language is superseded by the decisions above.

## Checks and remaining proof

Current repair: all fourteen focused retirement tests pass, including actual
eligible checkout deletion after disconnected/crashed owner expiry, other-view
retention, protected work and a brief reconnect revoking queued shell closes.
Remote framing/liveness (2), Session-viewer release (1), and prepared-launch and
authentication routing (2) tests pass. Direct OpenSSH lifecycle/failure checks (2, including twenty lifecycle cycles),
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, the Rust FFI build
and `swift build` pass. Swift linking emits SDK-version/Ghostty symbol warnings;
the build succeeds. Gate/CI own broader affected suites.

Native demo is still owed: local and real remote Desktop shell reconnect/Close,
orderly Quit delivery, Rex end-to-end Session viewing, lost connection and return,
and no unintended process/PTY/socket survivors. The private demo roots and input
contract are in `loo-717-demo.md`; no private demo provider/app was launched.
Headless framed-transport tests do not establish a physical remote-Machine or
native-host demonstration. Publication, merge and Task completion remain separate.
