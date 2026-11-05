# LOO-717 — Shell lifetime and Task Session viewers

Status: **Design revised November 5; implemented on the branch (pursue), awaiting gate and review.**
Jack Heart clarified that an open LF viewer of a Task Session retains the Task's
worktree, while a random shell in that directory does not. Jack then requested:
"ok. update the design then laucnh a new pursue". Build and review publication are
authorized; no shipping, installation or live-store migration. The Task stays at
its existing demo boundary while a separate Task-bound pursue Graph does this work.

This design supersedes the external-host occurrence/lease/settlement architecture
published in PR #1811 at b81df60579. Its earlier implementation evidence and design
remain at `b81df60579:scratch/loo-717-shell-lifetimes.md`. The complete November 5
LOO-498 handoff and this conversation's refinement are copied into
`rex-coordination.md`; paths in sibling checkouts are not required context.

## Accepted experience

- Desktop shells run on the Task's recorded Machine and checkout. Hide, Task
  switching and app exit preserve live shell execution until eligible cleanup.
  Reconnect attaches to surviving execution. Remote failure never exposes a local
  successor prompt. New Shell is explicit and never replays arbitrary commands.
- Close a Desktop shell ends that shell and its ordinary jobs. Undo restores
  identity/history, not a new process. LF automatically cleans up only its
  Desktop-owned shells. Rex owns its own shells, layouts and restoration.
- **An open LF viewer of a Session associated with the Task retains its checkout.**
  Selection, focus and native tab visibility are irrelevant while that viewer
  remains open. Passive viewing also counts; input control is a separate claim.
  Any remaining qualifying viewer keeps the hold after another viewer exits.
- **A random shell whose cwd is the worktree does not retain it.** Do not scan
  cwd, titles, editable labels or native window inventory to manufacture a hold.
  Running LF to view the Task Session establishes a real Session-view relationship;
  merely running an unrelated command in its directory does not.
- Jack's latest clarification identifies the LF viewer process as the boundary.
  Its confirmed exit releases its hold. A Rex tab left behind after that viewer
  exits does not preserve the hold just because the tab or saved screen remains.
  This supersedes the stronger retained-native-block interpretation in the LOO-498
  handoff. Agent/provider exit and viewer exit are separate events.
- Unknown viewer liveness is not confirmed exit. A lost remote connection does
  not by itself establish that the LF viewer process is gone. Use existing
  Session/process/attachment evidence and preserve conservative cleanup when
  evidence is unavailable; do not introduce a host registry to bridge the gap.

## Completion and cleanup

Unfinished Tasks do not acquire completed-Task checkout cleanup. Completion keeps
Task/conversation history and permits follow-up while a Session viewer remains.
After the last qualifying viewer/accepted Desktop hold releases, apply ordinary
absence cleanup: eligible unviewed idle agents use the existing two-minute policy;
completed-Task Desktop shells and checkout can retire at fifteen minutes total.
Keep explicit-Close behavior where already accepted. A renewed viewer cancels a
queued retirement. Confirmed Desktop process exit releases its views without an
app relaunch (accepted option 1a); do not require the dead app to report again.
On the built branch this holds for local Tasks. A remote Task's owner sees Desktop
quit as an SSH hangup, indistinguishable from network loss, so it stays unknown
until the Session is viewed again (settled below).

Requested final work, drafts, pending input and running Graphs still protect
execution. Work finishing after an elapsed deadline does not start another
fifteen-minute wait. Use wall-clock deadlines. Ordinary shell output alone is not
an agent turn. Cleanup discards eligible uncommitted files, preserving existing
committed-ref, primary/persistent-checkout and unrelated-resource safeguards.
LF never closes Rex shells, even if they still have cwd in the deleted checkout.
History survives; automatic deleted-checkout reconstruction stays outside scope.

## One walkthrough

Complete a Task with its LF conversation viewer open in Rex. Hide that Rex tab:
the viewer still retains the checkout. Open an ordinary shell in the same
worktree, then close the LF viewer. With no other Task Session viewer or protected
work, ordinary cleanup becomes eligible. The ordinary Rex shell stays Rex's;
it neither retains the checkout nor gets a Close request from LF.

Repeat with two LF viewers: closing one retains the checkout. Repeat with a
network failure: unavailable liveness cannot count as confirmed viewer exit.
Recovery either confirms the surviving viewer or its exit, without restoring
LF host-occurrence rows or requiring Rex-native terminal metadata. Reconnection
must not replay provider input or transfer input ownership automatically.

## Actions → model → APIs

| Action | LF concept | Implementation direction |
| --- | --- | --- |
| View a Task Session | Existing Session-associated LF viewer | Reuse attachment and Process identity; viewing need not claim input |
| Hide the native tab | Same viewer still open | No host focus/away report |
| Open a shell in the worktree | Ordinary shell | No checkout retention registration |
| Exit the last LF viewer | Release that Session-view hold | Confirm viewer exit, recheck other holds, schedule existing retirement |
| Complete Task / last hold releases | Task retirement intent | Keep one completion/checkout/branch-pinned executor |
| Cleanup becomes eligible | LF-owned execution and Git resources | Settle Desktop shell owners and requested work; never close Rex shells |

The serving owner already knows live viewer connections (`transport.rs` updates
`Activity::view` from live viewers), and `transport/lifetime.rs` records viewed
state on the existing process ledger. This is evidence to reuse, not proof that
one cached boolean covers all viewer exits, network loss or owner replacement.
Trace the real LF viewer process's Session association through the existing
attachment/handoff lifecycle and repair that boundary where needed. Do not keep
an agent alive merely to remember a dead viewer, or infer viewer death from agent
exit. Test multiple viewers and stale release after a new viewer attaches.

Remove external-host persistence and its write contract: no `task_host_occurrences`
for Rex, lease-table exception, sidecar, copy registry or host creation journal.
Delete or narrow the generic `task presence`/`TaskHostOwner` machinery rather than
leaving an unused external-host path. Keep any needed Desktop state in existing
Desktop/Session ownership, not a replacement universal host registry. Revise the
one unreleased Task migration in place. Keep LF's Task/Session/Process identities,
Desktop-owned shell receipts and completion-pinned retirement intent.

No Rex native inventory, block-to-Task metadata, native-shell settlement handshake
or reverse host-process probe is needed solely to retain a live LF Session viewer.
Native host metadata is no longer a prerequisite for this design. Input claims,
provider identities, local/remote Machine routing and newer-viewer fencing remain.

## Relationship to LOO-672 and LOO-498

- **672 makes conversations ready in Desktop:** automatic open/prefetch, bounded
  warmth for unfinished Tasks, explicit input takeover and Close. Its current
  accepted scope includes repository and Wave primaries. It is stacked on 717;
  preserve its shared Session preparation and retention integration. Its warm
  Desktop views feed this lifetime contract, not another cleanup authority.
  Completed Tasks release on departure under the existing accepted Desktop policy.
- **498 renders layouts into Rex and other native hosts:** explicit open is fresh;
  the native host owns subsequent arrangement and restoration. Config owns future
  templates; LF DB owns sticky Desktop layouts only. It must not persist external
  host occurrences or close Rex shells. An LF Session viewer launched in Rex
  participates through LF's existing Session/process identity.
- **717 owns shell execution and cleanup:** Desktop shell survival/Close/reconnect,
  viewer-based retention and coordinated retirement of completed Task resources.
  These are complementary responsibilities. Do not copy 672's prefetch policy or
  498's native layout implementation into this Task.

November 5 read: 672's status was shipping and its latest native demo recorded
repository/Wave/Task background startup; that is not authority to operate it.
498's handoff reports PR #1814, code b1864599e76b2eca74a3a03fd8ffc41d287a1682,
notes f129371904. Neither establishes this revised cleanup integration.

## Implementation — November 5 (pursue)

Viewer evidence is the session owner's own connection table. Each owner records
`processes.viewer_hold` (`viewed` | `unknown` | NULL) beside the existing `viewed`
flag; nothing else is persisted and no host identity is named.

- `viewed`: an attached viewer that has not sent `leave` and is open or inside
  the existing 60 s reconnect grace. Hidden/unfocused native tabs keep the relay
  open, so they hold. Passive (non-controlling) viewers count.
- Clean `leave`, and a local connection outliving its reconnect grace, are
  confirmed exits. A local relay never resumes, so a lost local socket is exit.
- A viewer attached under SSH (`ssh: true` on attach, from `SSH_CONNECTION`) that
  drops without `leave` is `unknown` after its grace, and at owner exit. Any fresh
  attach to that owner supersedes the unknown (recovery confirms a viewer).
- Task holds read only the Session's current transport owner and Desktop shell
  owners. A completed owner holds only with `unknown`; an owner that ended
  without writing (crash, kill) keeps its last value and reads as unknown.
- Idle agent ending stays on live `viewed`; `unknown` never keeps an agent alive.
- The retirement executor exits while any hold exists. An owner whose hold
  clears wakes the executor for its Tasks' waiting intents; completion and
  landing schedule as before. Deletion rechecks holds immediately before effects.
- Desktop departure and window close detach its terminals with Ctrl+] (a
  deliberate `leave`); quit hangs them up. No presence reports, quit budget or
  app delegate.

Check (Nov 5): focused lib `task_retirement::`, `ops::task::retirement`,
`transport::lifetime`; integration `session_viewers_retain…`,
`waited_land_retains_intent…`; Swift Workspace/WorkDestination/DTO/Ghostty input
suites; fmt, clippy, architecture, inventory — pass. Gate owns affected suites.
Compress (Nov 5): `transport::lifetime` lib, `transport_tests session_viewers_retain…`, clippy — pass.
Realign (Nov 5): docs/plan reconciled, no code change, no rerun.

### Delete — do not maintain

- `task_host_occurrences` table/triggers in `task_workspace_lifetimes.sql`.
- `lf task presence`, `HostUse`, `TaskHostOwner`, `parse_task_host_owner`,
  `observe_task_host`, `retaining_hosts`, `release_exited_host`,
  `owner_host_hold`, `owner_host_closed`, `retirement_hosts_settled`,
  `retirement::{observe, reconcile_hosts, owner_evidence}`.
- Hidden `lf machine process`, its SSH resident routing and the
  `ProcessIdentityEvidence` serde derive.
- DTO fixtures `task_host_owner.json`, `task_retirement.json` and their Rust/Swift tests.
- Swift `TaskHostOwner`, `TaskRetirement`, `RegistryQuery.taskPresence`,
  `SessionsWorkspace` presence/revision/quit reporting, `DesktopAppDelegate`
  quit release, the retirement banner, and `WorkspaceRetentionTests` presence cases.
- `ssh_shells` remote-host-exit retirement test; host cases in `retirement_tests`.
- transport README presence/native-settlement recipe.

Preserve: Desktop shell create/attach/Close/jobs settlement, completion-pinned
retirement intent, requested-work/draft/pending protections, Git safeguards.

## Draft usage copy for the next implementation

> Keep a Task Session viewer open to retain its worktree. Hidden or unfocused
> viewers count. Closing the last viewer lets a completed Task follow its normal
> cleanup grace once requested work finishes. An ordinary shell in that directory
> does not retain the worktree. Loopflow cleans up its Desktop shells; Rex shells
> remain yours to manage in Rex. Task and conversation history remain after cleanup.

Update Swift/transport user docs with the implemented contract; remove the old
Rex presence/native-settlement recipe. No new agent skill or user cleanup ritual.

## Next proof and delivery

1. A real LF Session viewer retains a completed Task checkout while hidden; a
   second viewer survives the first's exit. Passive viewers count without takeover.
2. Closing the last viewer releases its hold; an unrelated shell with the same
   cwd neither blocks checkout retirement nor gets terminated by LF.
3. Agent idle ending, owner replacement and network loss do not fabricate viewer
   exit. Confirmed Desktop exit settles its presentation without app relaunch.
4. Retention survives stale detach/exit races; renewed viewing and reopened Task
   cancel queued deletion. Requested work/drafts/pending input still protect work.
5. No external-host records are persisted. Existing local/SSH shell continuity
   and actual cleanup tests still pass. Formatting, Clippy and affected checks run
   under repository cadence; previous architecture tests must change with it.

## Remote viewer hangup — settled by accepted decisions (concept review, Nov 5)

Quitting Desktop, crashing it, or closing a Rex tab on a remote Task's viewer hangs
up its SSH relay; the owner cannot tell that from network loss. The build keeps
that hold `unknown` until any viewer attaches again (former option a). This is what
the accepted decisions already imply, not an open choice:

- Jack's 1a steer: "network loss and Rex client disappearance alone remain unknown";
  Jack's Nov 5 request: preserve "unknown remote liveness" and remove the generic
  settlement architecture, including the reverse `lf machine process` probe that
  previously confirmed a remote Desktop's exit. An expiry (former option c) is
  lease-based release, which 1a rules out.
- The LOO-498 handoff: keep cleanup conservative while evidence is unavailable.
  The cost is retention only; no work or history is lost, and viewing the Session
  again resolves it.
- Confirmed Desktop exit recovery holds where the evidence exists: local Tasks
  (socket close is the viewer's exit) and every deliberate Desktop departure or
  window close (Ctrl+] leave), local or remote.

Carried to review, not blocking: an orderly Desktop quit could also send Ctrl+] to
remote viewers (former option b). That reverses this design's deletion of quit-time
release and needs a bounded quit wait (the byte must cross SSH before hangup), so it
stays out of this pursue. A relay-side leave on terminal hangup would cover Rex and
crashes too, but `ssh` inherits the tty today and receives the same SIGHUP.

Concept review also checked: viewer exit is never inferred from provider exit (the
viewer itself returns on `ChildExited`, so the release follows a real viewer exit;
a Rex tab left behind is not a hold); no external-host persistence remains in
Rust, Swift or docs; Desktop departure release is Desktop's accepted host policy.
Next proof stays the list above; real Ghostty detach timing and Rex end to end
belong to demo.

Run the explicitly requested pursue Graph through publication, leaving review,
installation, merge and Task completion separate. Do not run past the Task's demo.
