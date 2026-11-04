# LOO-717 — Native shells and completed-Task cleanup

Status: **Accepted; implemented, installed acceptance incomplete.**
Jack Heart approved building and review publication on November 3, 2026, then
selected confirmed retaining-host exit recovery (option 1a) on November 4 and
requested implementation. The existing feature Graph stops at interactive demo.
No Auto-Ship, merge, installation or independent LOO-498 launch is authorized.

The full earlier design, simulated review, failed probes and dated evidence are
preserved at `e7fd5db44f:scratch/loo-717-shell-lifetimes.md`. This plan supersedes
its unimplemented November 4 section without changing the accepted contract.

## Accepted experience and owners

- Rex owns its native layouts and shells; Desktop owns its own presentation and
  shells. Shared lf Task/Session identity, Machine routing and retention serve
  both hosts equally. No mirrored Rex split tree, mandatory shell relay, live
  ordinary-shell attachment in Rex, or process migration between hosts.
- Desktop shells run on the Task's recorded Machine in its recorded checkout.
  New Shell is explicit. Repeated create/attach uses the receipt's execution
  identity; uncertain startup never silently replaces it. Reconnect preserves
  surviving execution. Remote failure, authentication loss or shell exit never
  opens a local successor prompt in the Task pane. No arbitrary command replay.
- Close a Desktop shell ends it and its ordinary jobs. Hide, Task switching and
  app exit detach presentation and preserve execution until eligible retirement.
  Undo restores the ended identity/history, not a new shell process. Rex retains
  native detach/close semantics. A saved screen is not live shell state.
- Hosts own retention/release policy. Desktop retains selected Work, releases on
  departure (including unfinished Tasks), and explicitly closes on repository
  window Close. A separate retaining occurrence keeps shared resources alive.
- LOO-709's owner remains the sole agent-ending authority: two minutes of eligible
  unviewed quiet, measured in wall time. Active requested work, drafts, pending
  input, native turns, running Graphs and headless Runs protect execution. Task
  membership alone does not. Ordinary shell output is not an agent turn; typing
  lf inside an ordinary shell does not change its creation purpose.

## Retirement contract

| Situation | Result |
| --- | --- |
| Task unfinished | No completed-Task checkout retirement |
| Any retained occurrence | Keep shared workspace resources |
| Last host releases as away | Begin fifteen-minute workspace absence grace |
| Two eligible idle minutes | Agent may end; shells/checkout remain |
| Return before retirement | Cancel deadline; later release starts a fresh interval |
| Last active occurrence explicitly closes | Skip grace once requested work settles |
| Requested final work runs beyond the deadline | Wait; no second fifteen-minute grace when it settles |
| Exact retaining instance is confirmed exited | Release as away and start ordinary grace without relaunch |
| Lease expires, network fails, or Rex client disappears | Unknown; never infer retaining-server death |
| All native and destination resources settle after eligibility | Remove eligible checkout/branches and retain Task/conversation history |
| Task reopens or placement/completion changes | Revoke the old pinned cleanup intent |

Jack selected disposal of uncommitted tracked edits and untracked files during
eligible completed-Task cleanup. Preserve existing safeguards for primary and
persistent checkouts, changed/unmerged committed refs, moved remote refs, other
Tasks and unrelated resources. Ordinary resume during retained-checkout grace
remains; automatic restoration of a deleted checkout is outside scope.

Use one durable retirement intent pinned to completion, checkout and branch.
Host writes and destructive passes share a Task lock. Prepare existing landed
Git deletion, then request settlement, wait on native resources and destination
Session/shell owners, recheck the intent, and apply deletion. Partial failure
keeps a pending reason and retries remaining effects on the next host report or
scheduling call. Active-work waits stay in the executor; recorded cleanup errors
end that run. This is not unattended retry after every failure. Cancellation is a fence
between effects, not rollback of external Git/process effects already underway.

## Confirmed exit implementation — November 4

Extend the existing revisioned host observation and Task migration, not a new
host registry, daemon or cleanup authority. The optional `TaskHostOwner` carries
`machine_id`, `pid`, kernel `started_at` in Unix seconds and `settles_on_exit`.
Desktop supplies its own process on its source Machine, even for remote Tasks.
Separate Desktop process instances have distinct occurrence IDs; a new app
cannot overwrite another instance's retention.

The existing retirement executor stays alive while verifiable retaining owners
remain, even when retention clears its cleanup deadline. It uses the existing OS
process identity reader locally and a bounded, Machine-validated SSH read remotely.
Remote observation needs an existing route from the execution Machine back to
the retaining Machine. Missing routes, malformed/failed responses and network
loss are unknown; no reverse route, credential or installation is created here.
PID plus start time distinguishes a reused PID; unreadable process evidence is
never death. The source's Machine identity prevents reading the same PID on the
wrong Machine. Network probes happen outside the Task mutation lock.

Positive exit updates only the inspected occurrence, revision and owner. A newer
return wins over an old exit read. Repeated exit observations do not restart the
grace, and delayed reports from the exited instance cannot revive its hold.
An exited historical occurrence does not veto a later instance's explicit Close.
Normal departure and confirmed exit converge on the same retirement intent.

Exit settles native resources only when the adapter declares they die with the
retaining process. Desktop's presentation qualifies because shells are owned on
the destination. A Rex retaining server's death does not establish that every
native job ended: its adapter must separately settle that Task's resources.
Rex client loss never substitutes for server death. Destination owners always
prove their own job settlement; process exit alone cannot delete the checkout.

Clean Desktop quit remains away plus native settlement, with one five-second
reporting budget across destinations. Quit fences window Close and heartbeats;
a prior explicit Close keeps its policy. A failed request is unknown to its caller,
but a lost reply does not undo a release committed at the destination. Retained
and unknown reports clear earlier native settlement. Native settlement does not
mean explicit Close and does not shorten absence grace.

## Delete — do not maintain

Already removed: local prompt tail in `buildWorkspaceShellCommand`, separate local
Desktop bootstrap, remote scripted-command refusal, termination via replacement
of the Undo slot, `retirement_hosts_closed`, and conversion of phase=closing into
`closeWorkspace()`. Preserve one destination shell path, literal initial argv,
login configuration, shell integration and retained history on the surviving path.

Confirmed-exit recovery also removed the indefinite hold after proven owner death
and the executor's unconditional exit when retention clears due_at. Desktop now
uses per-instance occurrences. Its old UserDefaults revision keys and wall-clock
revisions are unnecessary: the retained workspace owns a monotonic counter for
that process's lifetime. Hosts that restore the same occurrence still persist
their revisions. Unknown-without-owner and failed-read protections remain.

No remaining predecessor path needs deletion. Executor startup and continuation
share `needs_executor`; host reports and reconciliation share `hold_retirement`
inside their transactions, canceling queued retirement without undoing explicit
shell Close. Keep one unreleased `task_workspace_lifetimes.sql` migration.

## Implementation/proof remaining

- Focused implementation checks are complete: exact process exit, stale-revision
  races, other retaining instances, native job settlement after server exit,
  requested work/drafts/pending input beyond the deadline, and cleanup without
  another Desktop report. Real loopback SSH proves a failed remote read is unknown,
  then the same executor observes exit after reconnect without fresh presence and
  starts grace. Composed local Git fixtures prove deletion after job settlement;
  the SSH exit fixture does not delete a remote checkout.
- Gate owns affected suites, migration-frontier/DTO checks, CLI dispatch,
  formatting, Clippy and broader completion/landing coverage. Relevant Rust scope:
  store::sqlite::task_retirement, ops::task::retirement, transport::lifetime,
  machine/SSH CLI, dto_fixtures and transport_tests::ssh_shells. Owner README also
  requires CLI graph compilation and Machine global-command boundaries.
- Demo owns installed Desktop quit/crash/relaunch, sleep, input/TUI preservation,
  failed Close presentation, execution-owner failure and physical remote-Machine
  recovery. Rex acceptance depends
  on LOO-498's independent native integration. Logout/reboot require a disposable
  OS environment. No saved preview or loopback fixture proves installed survival.

November 4 reconciliation found no bounded runtime repair. Main through
`ac8e088f5f` supplies native status restoration across reconnects; the branch retains
that path and LOO-709's existing active-work/draft protections. No new product
decision is needed. LOO-498's published PR #1814 is independent delivery evidence,
not proof that its native adapter satisfies this contract.

## Prior evidence and limits

`781e5dc5b4` passed workspace-retirement policy tests (6), the exact waited-land
regression (1), Swift build and headless WorkspaceRetentionTests (11).
`2abcfc28cc` added composed Git retirement (5) and real loopback SSH fixtures:
twenty lifecycle cycles preserve shell PID/variable, reconnect after SSH loss,
close owned jobs and inspect process/PTY/socket/receipt settlement; separate tests
cover authentication failure, stale checkout and normal exit. These are earlier
passes, not a passing result for the current changed tree.

Apple's sshd cannot nest its sandbox inside test_network.py; run the named SSH
fixture directly with --ignored on macOS. Linux CI runs it in ordinary isolation.
Use generated keys and disposable stores; no installed registry or SSH service.

LOO-498's prior native probe preserved detach/reinsert continuity, but server
restart kept block IDs while replacing programs with login shells; invalid cwd
fell back to home. Its inspected APIs did not establish live cross-session moves.
Those limits still require real adapter proof. No additional native claims follow.

## Coordination and boundary

LOO-498 owns Rex mapping, layouts, opening/move/copy, templates and its retention/
cleanup adapter. Share exact API needs in the existing coordination note/comment;
do not launch its work. LOO-593 owns broader Desktop-server architecture. Preserve
LOO-709/719's active-work/idle policy and installed-proof boundary, LOO-714 routing,
LOO-711 presentation and LOO-672 conversation recovery.

No mobile, Tailscale setup, public hub, general network server, compulsory Rex
backend, process migration, arbitrary replay or workload recipes. Review publication
belongs to the feature Graph after implement; stop at interactive demo.

## Concept review — November 4, after implementation

Outcome: **no design change and no open product choice.** Jack Heart's accepted
1a resolves the earlier mismatch: a confirmed dead retaining instance no longer
requires a fresh report from that instance to release its hold. The implementation
uses the existing host observation and retirement intent; no additional lifecycle
or user action is needed. The review page now distinguishes the earlier behavior
from the implemented path and from installed acceptance still outstanding.

The normal interaction remains select, work, then leave. Departure releases
retention; explicit Close also skips workspace grace. A crash converges on
departure only after exact-owner exit evidence. A lost connection cannot do that:
recovery needs an existing route to observe the retaining Machine, or a fresh host
report. Return cancels cleanup, and an old exit observation cannot release another
instance. These are consequences of the accepted contract, not new approvals.

Keep release separate from native settlement. A Rex server can exit while its
jobs survive; merging those facts would discard the accepted protection. Desktop
presentation can settle independently because destination owners retain and end
its shells. Requested work, drafts and pending requests remain with their existing
owner policy. Deletion still requires the pinned completion, settled resources
and existing Git safeguards. Task and conversation history survive it.

The usage guidance in `swift/README.md` and `transport/README.md` already states
this experience; no skill or API redesign is implied. Next proof remains gate's
affected checks and demo's installed crash/quit/relaunch and physical remote
recovery, including a surviving host during disconnect and another instance
returning during cleanup. Existing focused results are not installed acceptance.

Check: November 4 realignment — `git diff --check` and `lf context` passed; documentation only, reusing compression's `cargo test -p loopflow --lib retirement` (18 passed) and `scripts/test_desktop.sh --jobs 2 --filter WorkspaceRetentionTests` (11 passed, Desktop built). Earlier exact SSH (`--ignored`), DTO and migration checks passed; SDK/linker warnings remain. Broader checks belong to gate; installed proof belongs to demo.
