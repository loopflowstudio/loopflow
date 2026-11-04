# LOO-717 — Native shells and completed-Task cleanup

Status: **Accepted; implementation present, acceptance incomplete.**
Jack Heart approved implementation and review publication on November 3, 2026,
after design review. The delivery boundary is interactive demo; shipping, merging
and installation are not approved. Reconciled November 3 against `9ab296eeff`,
which includes main through `aef0c0a5cd` and LOO-709's `ed983ac176`.

`scratch/explore.html` is the historical simulation, not the implemented terminal
experience. The accepted host-native design supersedes kickoff's universal shell
portability and independent indefinitely retained-shell policy.

## Accepted experience

Jack Heart wants Loopflow to work well with Rex and avoid duplicate work.

- **Rex owns its active layouts and native shells.** Loopflow does not maintain
  a second Rex split tree or represent Rex-native shells as Desktop shells.
- **Desktop owns its active layouts and shells.** A Desktop shell can execute
  on the Task's remote Machine and survive app exit; its workspace and recovery
  UI still belong to Desktop. Shared Rust mechanisms may implement its owner.
- **Work remains shared:** Task identity, recorded Machine/checkout, existing LF
  agent conversations, and semantic workspace-opening intent. A saved template
  or intent is distinct from the native host's current layout. LOO-498 owns host
  workspace opening, moves/copies, templates and native tool mapping.
- **Opening the same Work in another host is not process migration.** Reuse
  existing host placement; create native shells only when explicitly requested.
  Keep LF conversation identity and input authority; do not clone providers.
  Files/Browser widgets and unsaved view state need not transfer exactly.
- **No live Loopflow-shell attachment in Rex for now.** Jack explicitly excluded
  that optional feature. No shell relay, shared shell-identity dependency or
  acceptance requirement for this path. LF agent-conversation attachment remains.
- **Close a Desktop shell tab ends that shell and its ordinary jobs.** Hide,
  switching Tasks and quitting Desktop retain live work. Undo may restore an
  ended tab/history, never resurrect a process. Rex keeps its native semantics:
  closing a native terminal ends its program; detach retains it.

Shell execution must always stay on the Task's recorded Machine and checkout.
Remote exit, auth failure or disconnection never produces a local prompt in that
Task's pane. Reconnect is distinct from New Shell; no replay of arbitrary commands.

## Completion, follow-up and retirement

Jack Heart specified this sequence on November 3:

1. Completing a Task keeps its Sessions available for questions and follow-up.
   Keep it open while Jack is selected into/using the Task.
2. After leaving the Task, detach its view and allow a short grace period before
   automatically closing it. Jack selected **15 minutes** on November 3.
   Returning cancels the timer;
   a later departure starts a fresh interval.
3. Coordinate cleanup of execution, shells, worktree and remote branch while
   preserving Task/conversation history. Jack's later working proposal separates
   agent ending at two minutes from shell/checkout cleanup at fifteen minutes.
4. In Rex, using the Task's windows counts as keeping it open. Jack expects
   closing those Task windows to allow worktree/branch cleanup; a closed or
   fully idle Task Session is another way to reach cleanup eligibility.
5. Jack subsequently confirmed that requested final work must finish even when
   the Task is complete and its window is closed or left. Wait for active work
   before cleanup; neither completion nor leaving cancels that work.
6. Jack then selected aggressive cleanup by default, canceled by Desktop or Rex
   retaining views. Retention is a host request, not an automatic property of
   unfinished Task membership. Jack accepts ending live Sessions on explicit
   Close, after requested work finishes. Preserve Task/conversation history;
   closing an unfinished Task's view does not retire its checkout or branches.

Accepted timing direction from Jack Heart, included in the build approval: after leaving a completed Task, an
eligible idle agent ends under the existing two-minute rule; checkout and shells
remain for another thirteen minutes, reaching fifteen minutes total. This replaces
the draft simulation that kept agents live for the whole grace. Active work,
drafts/pending requests and host use still protect their resources. Before minute
fifteen, return can resume the same conversation in the retained checkout. The
exact continuation UI is proposed. Jack subsequently questioned what post-cleanup
“return” meant. Checkout resurrection was an agent-introduced hypothetical, not
a requested feature; leave it outside required scope rather than making it a
product decision needed before implementation.

Jack Heart further specified that completed-Task cleanup should proceed even
with uncommitted work. Tracked edits and untracked files in the disposable Task
checkout do not veto cleanup; they can be discarded when eligible cleanup removes
that checkout. Active requested work and host retention still delay cleanup.
This supersedes the proposed dirty-checkout retention rule. It does not select
new behavior for unmerged commits, moved remote refs or unrelated checkouts.

Implementation contract (routine engineering details are delegated by the build approval):

| Situation | Result |
| --- | --- |
| Task unfinished | Existing lifecycle; no completed-Task retirement timer |
| Completed Task selected/used in any host | Keep workspace, Sessions, shells and checkout |
| Leave a completed Task | Detach that view; begin the grace period when no other host is using it |
| Two eligible idle minutes away | End the agent; retain conversation history, shells and checkout |
| Return before minute fifteen | Cancel retirement; reuse the live agent or resume the conversation in the existing checkout |
| Follow-up agent turn still running | Defer cleanup until it settles; an elapsed workspace deadline does not require another fifteen-minute wait |
| Explicitly close the last Task workspace/window | End live Sessions once requested work settles; completed-Task resources become eligible for coordinated cleanup |
| Another Desktop/Rex occurrence is still in use | Keep shared Task resources; one closed copy does not end another |
| Completed, absent and settled for fifteen minutes total | End remaining eligible execution and clean shells, host workspaces, checkout and branch |
| Task reopened/new accepted work | Cancel the previous completion's cleanup intent |
| After retirement | Preserve history; automatic checkout restoration is outside the requested scope |

Jack accepts ending live Sessions on explicit Close once requested work settles.
Jack Heart clarified the boundary: lf treats Desktop and Rex equally. A host
retaining a Session keeps its execution retained; lf does not interpret native
tabs or selection. Each host decides when to release retention. Desktop should
not prioritize retaining completed Sessions; the current working interpretation
is to release their hold on departure, allowing the two-minute idle-agent policy.
Rex retention is decided by Rex's adapter, not a Desktop rule in shared lf logic.
The fifteen-minute shell/checkout cleanup remains separate and must respect
other host retention and active work. Supported host signals need implementation
proof; a stored layout alone is not a live retention request.
Cross-client retention implementation remains an engineering choice. His direction
is coordinated automatic cleanup after the post-completion follow-up window.
"Fully idle" is not terminal-output silence: selected use and active agent work
must count. A development server's ongoing shell output must not preserve a
completed, abandoned-by-the-view workspace forever; Task-owned shells are part
of the requested retirement. Preserve active agent drafts and pending requests;
uncommitted checkout files do not veto completed-Task cleanup.
Reconcile this with LOO-709's supported protection rather than adding a competing
agent-idle policy. Report retained work when safe retirement cannot settle.

Closing a Session here ends its live execution/attachment and retires its
workspace presence. It does not erase native/provider history or Task provenance.
Idle process ending from LOO-709 remains distinct from this coordinated Task
retirement: an early idle provider exit alone does not delete its Task checkout.

### Integrated agent policy

LOO-709's merged owner policy remains the agent-ending authority. Its default is
two minutes of unviewed quiet, measured in wall time; output, renewed viewing and
held work affect that clock. Drafts, pending requests, native turns, headless Runs
and running Graphs protect execution. Task membership alone gives no exemption.

This branch adds explicit host retention and settled-work Close/retirement to that
owner policy. Desktop releases on departure for unfinished as well as completed
Tasks; this is the delegated implementation choice recorded in `questions.md`.
Shell purpose is fixed by its creation receipt, so typing `lf` inside an ordinary
shell does not enroll the shell in agent idle ending or binary rollover.

Ending an AgentProcess leaves the durable Task Session open. Screen/provider
history supports conversation resume, not reconstruction of arbitrary shell state.
LOO-719 owns the earlier policy's installed macOS proof; this branch still needs
its composed lifecycle demonstration.

## Host authority and minimum coordination

Rex is authoritative for its live layout, terminal programs, and native close
operations. Desktop is authoritative for its own layout and shell presentation.
Loopflow's Task lifecycle coordinates shared-resource retirement. It needs only
Work-to-host occurrence references and evidence that a Task is in use, left,
closed, or unavailable, plus a way to request cleanup of its owned occurrences.
These references are not a replicated native layout or second shell registry.

Presence must include other windows/copies/hosts; LOO-498's Task-to-tab and
Wave-to-session mapping means cleanup cannot blindly close an entire Rex Wave
session. Close only the Task's associated occurrences and retain other Tasks.
Warm surfaces, an open socket and an idle server's last-focused block are not
proof that the person is looking at a Task. A lost remote observation is unknown,
not proof that all windows were closed. Validate supported Rex visibility/close
signals before claiming automatic cross-host retirement. Missing host observation
keeps a visible pending-cleanup reason rather than inventing absence.

All cleanup steps share one durable intent tied to the current completion and
resource identities. Use existing Task/Process/host authorities; no separate
mutable "Task is done" state. One executor claims the intent, rereads current
completion/use/activity, and makes partial cleanup retryable. A reopen or return
must win over an old queued timer before it performs destructive work.

Implemented cleanup order:

1. Recheck current completion, host use, active work, drafts and checkout/ref
   state. Save retained conversation/screen evidence through existing owners.
2. Ask each host/owner to close the Task's relevant workspace occurrences and
   end its Sessions/shells. Wait for observed settlement; failed termination is
   pending cleanup, not proof that execution is gone.
3. Use existing landed-delete preparation and deletion to remove the eligible
   remote branch, worktree and local branch. Allow removal of the eligible Task
   checkout with uncommitted changes. Preserve moved/unmerged committed heads,
   primary/persistent checkouts and shared resources under existing ref rules.
4. Record completion of cleanup and keep Task/conversation history accessible.

This is one coordinated operation, not an atomic transaction across processes,
Git and remote services. Show partial results and retry only remaining effects.
A completion can remain durable while resource cleanup is pending. Discarding
uncommitted checkout files is now selected for eligible completed-Task cleanup;
keep that scoped to its recorded checkout. No arbitrary process termination by cwd.

## Delete — do not maintain

The accepted cut already removed the local prompt tail in
`buildWorkspaceShellCommand`, Desktop's separate local shell bootstrap, the
remote scripted-command refusal and delayed termination through the Undo slot.
Keep one destination-owned Desktop shell path and preserve literal initial argv,
login configuration, host retention and ended-history behavior. Do not restore
those predecessors while adding failure coverage. No further production owner or
store is replaced by this acceptance-test pass.

Keep landed deletion and Task retirement on one merged-head check. Shell input
uses the purpose already fixed at launch; it does not reread the receipt for each
write. The tab model creates shell identity once, without a caller-supplied ID.
Local and SSH fixtures share Task/receipt setup, and SSH control commands share
one configuration path for checks and teardown. The old next-Close surface
assertion is removed with the Undo-slot termination policy it described.

## Implementation and evidence

- `ops/task/workspace.rs` gives local and remote Desktop shells the same
  destination owner. A receipt's insert winner starts execution; repeated create
  or attach reuses that identity. An unacknowledged startup remains unknown,
  never silently replaced. Existing receipts can show history after checkout
  deletion; fresh creation requires the recorded checkout.
- Packaged destination shell startup supplies personal login configuration,
  literal initial argv, inner PTY identity and Ghostty integration. The local
  prompt tail, separate local launcher, remote scripted-command refusal and
  Swift bootstrap were removed. Shell startup compatibility beyond focused
  fixtures still needs acceptance evidence.
- `TerminalJobs` settles the shell's OS-session jobs, including separate job
  control groups. Group-only termination failed the earlier disposable probe;
  intentionally daemonized independent programs remain separate ownership.
- Desktop Close requests termination and releases the surface immediately.
  Hide/departure detaches surfaces; Undo restores the same identity. Replacing
  the Undo slot no longer decides execution lifetime. Attachment exit retains a
  disconnected pane and any captured screen, offering Reconnect/New Shell;
  it does not claim the remote process died.
- Completion, Graph and landing callers schedule `ops/task/retirement.rs`.
  One migration stores shell receipts, native-host observations and a retirement
  intent pinned to completion/checkout/branch, with revision and revocation
  triggers. Per-Task locks serialize host reports and cleanup passes; one executor
  watches each intent. Host returns cancel pending deadlines and retirement Close
  requests. Task reopen or changed placement revokes the intent.
- Retirement prepares deletion before asking owners/hosts to close, then waits
  for process and shell-job settlement. `prepare_task_retirement` permits dirty
  file disposal while preserving primary/persistent checkouts and observed
  committed refs; remote deletion retains the existing lease. PR-less or
  unresolved delivery remains pending under the existing policy.
- Headless policy tests cover deadlines, return/reopen, multiple host reports,
  expired observations, active work, drafts and pending requests. The local
  transport fixture proves shell state across detach/reattach, one initial
  command, nested-conversation idle exclusion, Close and retained history.
  These tests do not demonstrate installed Desktop or Rex behavior.
- LOO-498's supplied probe found native Rex detach/reinsert continuity, but server
  restart preserved block IDs while replacing programs with login shells.
  Invalid cwd fell back to home. Its inspected move APIs did not establish
  cross-session live migration. These are prior probe limits, not fresh checks
  or native integration delivered by LOO-717.

Earlier source baseline, failed job-control probes and reference research are
preserved at `9ab296eeff:scratch/loo-717-shell-lifetimes.md`, under **Existing
mechanisms and evidence**. Main's LOO-709 and LOO-711 changes are integrated;
LOO-714 routing remains an overlap constraint, not a new change here.

## Simulated design review

Jack requested a simulated Mitchell Hashimoto review. See
`scratch/loo-717-design-review.md` for the agent-authored findings and primary
sources. It recommends proving the host retention/release boundary under client
loss, specifying two-stage cleanup deadlines, and using a real terminal demo.
These recommendations do not authorize implementation or add product decisions.

## Host adapter contract

`task presence TASK --occurrence ID --state STATE --revision N` accepts retained,
away, closed or unknown observations. Persisted increasing revisions fence delayed
reports. Retained leases last thirty seconds; expiry becomes unknown for workspace
cleanup. Away/closed observations persist. A live retained lease holds Session
execution; an expired lease is not an indefinite agent hold. Unknown observation
still prevents destructive checkout retirement. This conservative distinction
means an unavailable host can leave cleanup pending until it reports again.

Desktop polls every five seconds, releases on departure and reports closed on
repository-window Close. At `phase=closing`, a released host closes only its Task
occurrences and acknowledges closed. Both hosts use this same contract; LOO-498
owns Rex mapping, reporting and native cleanup. The shared API was sent in comment
`67cd130c-9104-48ef-b019-615e3e41e01e`; `rex-coordination.md` retains the boundary.

## Remaining work and evidence limits — November 3

- **Real SSH automation is implemented** in `transport/ssh_shells.rs`: disposable
  sshd, generated keys, separate Machine stores and the public remote shell route.
  Twenty cycles kill the SSH master, reconnect to the same shell PID/variable,
  close it and check process/PTY/socket inventories and settled receipts. Failed
  authentication, stale checkout and normal shell exit return without local
  execution or implicit replacement. This is loopback SSH, not a second physical
  Machine or an installed Desktop demonstration.
- Apple's sshd cannot initialize its sandbox within `test_network.py`'s sandbox
  (`ssh_sandbox_child: sandbox_init: Operation not permitted`). The two SSH tests
  are explicitly ignored by default on macOS and run directly with `-- --ignored`;
  Linux CI runs them in its ordinary isolated suite and installs openssh-server.
  No test sandbox exemption or system SSH configuration change was introduced.
- Composed retirement tests drive the actual cleanup operation against disposable
  Git worktrees and a bare remote: missing native-host acknowledgement leaves
  resources intact; return/reopen during closing cancels before the next destructive
  pass; unconfirmed shell-job settlement after recorded owner death retains the
  checkout; a locked worktree after remote deletion leaves retryable closing, and
  retry skips the already-deleted remote ref. Owner-death evidence is supplied in
  the fixture; an installed owner crash remains demo evidence.
- Gate owns the existing local twenty-cycle census, completion/landing regression,
  migration-frontier/DTO checks, Clippy and affected suites. Linux CI owns the new
  SSH fixtures under Linux process and network isolation. The new focused tests
  do not replace the existing Swift or agent-policy coverage.
- Installed Desktop quit/relaunch, native input/TUI preservation, failed Close
  presentation and remote recovery remain demo evidence. One host's state records
  do not prove the other's real adapter. Rex acceptance depends on LOO-498's
  independently owned integration; unavailable proof remains visible here.
- No new product decision is needed to finish these checks. Continuation UI during
  retained-checkout grace remains subject to demo judgment. Automatic deleted-
  checkout restoration remains outside scope.

## Demo and acceptance

Show local and actual remote Desktop shells executing on their recorded Machine
and checkout. Preserve a variable, unfinished command, foreground TUI and dev
server across Hide, switching and Desktop quit/relaunch. Reconnect to the same
live shell after SSH loss. Close ends only that shell and ordinary jobs; Undo
shows ended history and offers an explicit fresh shell. Failed auth, stale checkout
and owner death show honest states, never a local successor prompt.

Then complete a disposable Task while viewing its conversation. Ask a follow-up;
its Sessions, shells and checkout remain. Leave it and show pending cleanup.
After two eligible idle minutes, show the agent ended while checkout and shells
remain. Return before minute fifteen and resume the same conversation in that
checkout. Leave again: after 15 minutes of eligible absence,
close owned Sessions and shells, retire the Task's host occurrences, delete its
eligible worktree and branch copies, and retain readable Task/conversation history.
Repeat using Rex-owned shells/Task windows through LOO-498's integration; no lf
shell relay. Another active occurrence keeps shared resources alive. Closing the
last completed Task window exercises the explicit-close retirement path.

Required headless behavior checks (implemented coverage and gaps distinguished above):

- Local/remote Desktop create, repeated prepare, lost acknowledgement, attach,
  hide/reopen, app loss, normal exit, Close, failed owner and cleanup settlement.
- Real shell job control: background/stopped jobs, inherited open descriptors,
  independent owners and bounded termination. Twenty disposable lifecycle cycles
  inventory fixture-owned processes, SSH helpers, PTYs, sockets and receipts.
- Completion while selected; leave/return resets grace; multiple hosts/copies;
  active follow-up, pending request and draft retention; Task reopen supersedes
  queued cleanup; app/client loss; unavailable remote host; quiet native shell
  versus active agent work. Use a controlled clock for the policy.
- Complete and close/leave a Task while its requested final turn is still working,
  including silent tool work beyond both timer durations. Let it finish before
  cleanup; neither the two-minute agent policy nor Task retirement interrupts it.
- End owners before deletion; discard uncommitted files in the eligible completed
  Task checkout, while preserving moved/unmerged committed branch heads;
  retries settle remaining resources after partial process/host/Git failures.
  Exercise both Task-completion and PR-landing callers. Test unrelated Task and
  repository resources remain intact; the primary checkout is never removed.
- Swift headless view/interaction and DTO tests cover selected/away/closed evidence,
  pending/retired/error views and history continuation without launch on paint.
  Fake SSH checks argv only; use actual isolated sshd for remote lifecycle proof.

Gate covers affected suites, formatting and Clippy. Installed Desktop/Rex and
actual remote-Machine proof belongs to demo; logout/reboot scenarios require a
disposable OS environment. A saved screen does not prove live shell-memory survival.

## Coordination and exclusions

LOO-498 owns Rex-native layouts, mapping, opening/move/copy, templates and host
presence/cleanup adapter. Its earlier "no automatic cleanup on Task state changes"
proposal was superseded at this shared boundary by Jack's selected completed-and-away
cleanup. Selection alone still does not close an unfinished Task. LOO-593 owns broader
Desktop-server roles; do not silently turn the current app-owned UI API into the
execution owner. LOO-709/719 own agent idle-preservation behavior and its installed
proof. Preserve LOO-714 SSH/account settings, LOO-711 Task presentation and LOO-672
conversation recovery.

No mobile, Tailscale setup, public hub, general network server, compulsory Rex
backend, process migration, arbitrary history replay or C-style workload recipes.
The accepted boundary remains interactive demo, without Auto-Ship, merge,
installation or a separate LOO-498 launch.

Check: November 3 — `cargo test -p loopflow --lib ops::task::retirement::tests::` passed (3); `cargo test -p loopflow --lib ops::wt::tests::` passed (5); `cargo test -p loopflow --test transport_tests desktop_shell_` passed (2, twenty local cycles); `cargo test -p loopflow --test transport_tests ssh_shells:: -- --ignored` passed on macOS (2, twenty SSH cycles). `swift build --build-tests --package-path swift` and headless `DesktopPaneControlTests|WorkDestinationTests|WorkspaceRetentionTests` passed (95; revised companion-Close test rebuilt and passed separately). `cargo fmt --check`, `git diff --check` and `lf context` passed; earlier CI YAML parsing remains applicable. Gate/CI owns broad checks, Clippy and Linux isolation; demo owns installed Desktop/Rex and physical remote-Machine judgment.
