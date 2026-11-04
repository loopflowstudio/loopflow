# LOO-717 — Native shells and completed-Task cleanup

Status: **Approved for implementation** — Jack Heart, November 3, 2026.
November 3, 2026. Jack Heart selected host-native ownership, Close ends, and no
Rex attachment to live Loopflow shells in this delivery. He then specified a
completed Task's follow-up period and coordinated cleanup. Jack then said
“ok. lets build it!” Continue the existing feature Graph to its interactive demo;
this authorizes implementation and review publication, not shipping or merging. `scratch/explore.html` is the
current review artifact. This plan supersedes kickoff's universal shell-portability
proposal and its independent, indefinitely retained-shell cleanup policy.

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

### Reconcile the merged two-minute policy

Jack asked whether the newly landed all-Session cleanup should exempt active
Task Sessions. Inspection of merged `ed983ac176` on November 3 found:

- `config.rs::DetachedLimit::default` sets two minutes. The timer applies to
  unviewed, quiet terminal owners, not every Session after two minutes.
- `transport/lifetime.rs` resets grace on output/view changes, checks eligibility
  before ending, and rechecks queued input, output and viewer generations.
- `store/sqlite/sessions.rs::owner_waits_for_person` protects current turns,
  headless Runs and running Graphs (step ownership and ancestor relationships).
  `native_input::pending` protects drafts and uncertain editing state. Current
  provider activity protects pending requests and work even without output.
- The predicate does not inspect Task completion/state. An unfinished Task's
  otherwise eligible idle conversation can therefore end. A viewed conversation
  is protected from ordinary idle ending; binary replacement has a separate
  settled-work handoff path.
- Ending preserves screen/history and supports conversation resume. It does not
  itself retire Task shells or delete worktree/branches. LOO-709's merged proof
  covers source behavior; installed macOS acceptance remains with LOO-719.

The attachment-exit path in `store/sqlite/processes.rs` leaves durable Task
Session records open when their AgentProcess ends. Process ending therefore does
not itself mark the Task conversation completed or remove its primary identity.

Jack subsequently preferred aggressive cleanup canceled by a host retaining the
view, and accepted ending live execution on explicit Close. Use host retention
rather than a blanket Task-state exemption. Reuse the merged idle owner policy
and active-work protections; a host hold must be explicit evidence, not inferred
from a saved layout or an unreachable host. Final-work protection applies
independently of Task state. The default two-minute unviewed-idle fallback remains
useful; explicit Close can release execution once settled without a new grace.
Keep host-specific hold release in each host, with Desktop and Rex equal under
lf Session management; coordinate it with the 15-minute completed-and-away lifecycle. Do not add a second timer that accidentally makes
the selected retirement interval seventeen minutes.

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

Suggested cleanup order:

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

## Existing mechanisms and evidence

Source baseline is `2bc29a72e0`; `origin/main` at inspection includes LOO-709 merge
`ed983ac176`. The branch needs integration with current main before implementation.

- `SessionsWorkspace.newShell`/`shellCommand` launches local Desktop shells
  directly; remote shells invoke `task shell --checkout`. The latter validates
  placement with `ops/task/workspace.rs::pinned_checkout` and uses detached lf
  transport. Scripted Desktop commands currently refuse remote locations.
- `buildWorkspaceShellCommand` runs the initial remote command then a local login
  shell. Remove that fallback. A real destination shell may remain after an agent
  command exits, but a lost remote attachment must not become a local shell.
- `MultiplexerStore.closeShell` has one Undo occurrence. Replacing it in
  `SessionsWorkspace.closePane` releases the previous Ghostty surface/PTY. Remove
  that accidental termination trigger; an explicit Close owns termination.
- Pane `ShellTab` already stores id, location and optional `lf_process_id`.
  Closed panes are not sufficient execution discovery. Desktop-retained shells
  need durable identity and discoverable owner/exit evidence independent of panes.
  Do not put Rex-native shells into this Desktop execution registry.
- Existing transport owns detached PTYs, input claims, repaint and bounded
  reconnect output. Reuse it for Desktop where useful. A single Loopflow Server
  interface need not mean one process owns every PTY. Choose process topology
  from concrete reuse/failure evidence, not the name Server.
- `ProcessGroupGuard` kills only the child's original group. Kickoff's disposable
  interactive-bash probe left a background `sleep 60` alive in another group.
  Earlier probe attempts failed to parse echo or blocked on teardown; all probe
  processes were stopped and absence checked. This is contrary evidence to
  group-only cleanup, not a successful lifecycle test. Clean the owned OS
  session's ordinary jobs, close PTYs before waiting, and bound observation/reap.
  Intentionally daemonized independent programs have separate ownership.
- `ops/task.rs::settle_completion` currently records completion then invokes
  `cleanup_completed_task` immediately. That function retains PR-less artifacts,
  unresolved delivery, primary checkout and live/unresolved execution. Its caller
  lineage is excluded from execution blockers; the current interactive caller
  does not by itself guarantee retention. Task tests preserve a separate live
  Process and Session across completion. Change this entry point to participate
  in the selected grace/retirement lifecycle.
- `ops/pr_landing.rs::cleanup_landed_pr` also routes a completed Task through that
  cleanup. Both must converge on the same eligibility/intent, not independent
  immediate deletion paths. Taskless landing keeps its separate existing policy.
- `ops/wt.rs::prepare_landed_delete` preserves work beyond the merged head;
  `apply_delete` checks clean worktree and unchanged local head, deletes the
  exact observed remote ref with a lease, removes the worktree, then local ref.
  Reuse its authority and partial-failure handling. Dependent PR bases are
  retargeted before remote deletion. The selected dirty-checkout cleanup needs
  a scoped change here: retain ref checks while permitting uncommitted file removal.
- Desktop `SessionsView.swift` exposes selected Work separately from retained
  workspaces. That supplies local presentation evidence but is not a complete
  cross-client presence contract.
- LOO-709 / PR #1804 is done and merged. Its eligible idle provider ending retains
  history/screen and protects drafts, pending requests, native active turns,
  watches and headless Runs. Installed macOS acceptance remains in LOO-719.
  Its protections constrain coordinated retirement; reuse rather than replace.
- LOO-498's isolated Rex probe verified live detach/reinsert. After server restart,
  block IDs survived but the original program became a new login shell. Stable
  presentation identity is not execution continuity. Invalid Rex cwd falls back
  to home, so validate the Task checkout before direct native shell creation.
  Task windows cannot be assumed to migrate across Rex sessions with the inspected
  `move_window`/`move_block` APIs. LOO-498 owns those host capabilities.

Rex reference facts were checked November 3: `block.close` ends its program;
`session.destroy` hangs up all blocks; `session.detach` and `detach_block` retain
execution. An on-demand personal server uses the account's socket and exits once
its sessions/connections end. Personal-server package replacement need not restart
it. This does not promise survival of server death, logout or reboot.
Sources: https://www.superlogical.com/rex/docs/reference/api,
https://www.superlogical.com/rex/docs/reference/mac-actions,
https://www.superlogical.com/rex/docs/linux/server-modes,
https://www.superlogical.com/rex/docs/linux/upgrading.

## Simulated design review

Jack requested a simulated Mitchell Hashimoto review. See
`scratch/loo-717-design-review.md` for the agent-authored findings and primary
sources. It recommends proving the host retention/release boundary under client
loss, specifying two-stage cleanup deadlines, and using a real terminal demo.
These recommendations do not authorize implementation or add product decisions.

## Implementation shape and deletions

One coherent delivery for the accepted shell/lifecycle outcome. Jack approved
implementation after review. Resolve technical details within this contract,
prove the execution paths, and bring the result to the authored demo boundary.

1. Reuse destination shell ownership for both local and remote Desktop. Immutable
   shell purpose must exclude arbitrary shells from agent idle ending even after
   typing `lf`. Keep one Task migration for any shell receipts/cleanup intent and
   Work revision triggers. Process ledger owns liveness; panes retain references.
2. Cut Desktop creation, hide/quit recovery, Close and ended/disconnected states
   over together. A repeated creation request reuses its identity; attach never
   creates a replacement on failure. Preserve destination shell setup and inner
   PTY markers. Move necessary shell bootstrap to one packaged resource usable
   on the execution Machine. Preserve literal argv and personal shell startup.
3. Make completed-Task cleanup use one durable grace/retirement operation across
   completion and landing. Feed native-host use/closed observations into shared
   eligibility. Coordinate Rex-specific reporting/close actions with LOO-498;
   do not implement another Rex opener or mirrored layout.
4. Update Task/Session/shell presentation for disconnection, ended execution and
   completed cleanup. Preserve history and ordinary resume while the checkout
   exists; do not add automatic post-deletion checkout restoration. Report partial
   failures. Update strict Rust/Swift DTOs and fixtures together where needed.

Delete the local-prompt tail after remote attachment, the separate local/remote
Desktop lifetime branches, remote-only scripted-command refusal when the shared
launcher supports it, Undo-slot-driven termination, and exited-shell disappearance
that obscures its result. Replace immediate Task-completion/landing cleanup with
the common retirement path. Keep native Rex shell ownership. Do not build the
previous cross-host ordinary-shell attachment or named-workload service.

No second provider conversation, copied Task state, duplicate native layout,
idle timer for unfinished shells, command replay, silent home/local fallback,
deletion of changed committed refs, or cleanup of another Task's windows/processes.

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

Required headless behavior checks (add focused tests at affected owners):

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

Implement runs focused tests. Gate uses affected suites once, formatting and
Clippy; commands follow `scripts/testing.md`, inherited `LF_*` unset and stdin
closed. Installed Desktop/Rex and actual remote-Machine proof belongs to demo;
logout/reboot tests use disposable OS environments. No shutdown of this working
Machine. A saved screen is not proof of restored live shell memory.

## Coordination and exclusions

LOO-498 owns Rex-native layouts, mapping, opening/move/copy, templates and host
presence/cleanup adapter. Its earlier "no automatic cleanup on Task state changes"
proposal must be reconciled with Jack's later selected completed-and-away cleanup.
Selection alone still does not close an unfinished Task. LOO-593 owns broader
Desktop-server roles; do not silently turn the current app-owned UI API into the
execution owner. LOO-709/719 own agent idle-preservation behavior and its installed
proof. Preserve LOO-714 SSH/account settings, LOO-711 Task presentation and LOO-672
conversation recovery. No writes to Wave memory or other Tasks' checkouts.

No mobile, Tailscale setup, public hub, general network server, compulsory Rex
backend, process migration, arbitrary history replay or C-style workload recipes.
Jack's explicit build approval authorizes the next implementation run. Stop at
the interactive demo; no Auto-Ship, merge, install or separate LOO-498 launch.

Check: November 3 — source and merged-main lifecycle review; prior exploration interactions passed. This planning revision runs no runtime cleanup, build or acceptance tests.
