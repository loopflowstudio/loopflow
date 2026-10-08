# Run remote work against the host's Task store — LOO-412

Jack Heart selected the callback design during PR #1491 review on 2026-10-08 and
requested a fresh `pursue`, followed by another review here. This supersedes the
adoption-only scope of PR #1491 and the earlier decision to defer callbacks from
LOO-406. Implement and republish this PR; do not land or start unrelated
remote-work slices. Machine registration and global `--machine` come from LOO-411.

## Experience

```sh
lf --machine mini task create --title "Fix the parser"
lf --machine mini --task <task-selector> skill implement
```

The command executes on mini, but planning reads and writes call back to the
originating host. Creating the Task immediately makes it visible in the host's
ordinary Task store. Creation alone allocates no host or worker checkout, Session
or Flow. Selecting it for work prepares mini's checkout from the pushed branch;
repeated launches reuse its local execution record and checkout.

Task creation, editing, comments and planning reads use one host plan. The remote
machine's unrelated plans remain unchanged. Checkouts, PR execution, Sessions,
Processes, Workflow position, machine registration and account operations stay
on mini. In particular, `lf --machine mini machine add builder` still edits mini's
machine registry. The callback is a planning route, not a second execution driver.

## Accepted direction and related work

Jack's original proposal in LOO-406, October 7 at 22:17 PDT: “we could explore
something where if you do lf --machine X and then we also bring some sort of
backwards connection so taht your task store is the host store”. At 22:22 he
leaned toward routing through the online host even with Linear connected.
His October 8 PR #1491 review selects that approach for LOO-412.

LOO-406's later October 8 review selects one on-disk model and code path in both
modes, either repository-wide Linear synchronization or none, with local commits
and visible pending sync if Linear is unavailable. That supersedes its earlier
personal/shared authority split. This Task routes to the host's ordinary planning
operations; it does not implement another local planner or another Linear writer.
The host can save locally while Linear is unavailable; loss of the host connection
is different and must not silently select mini's own store.

LOO-406 / PR #1503 is actively being revised in its own checkout. Its settled
planning API and retained creation identity are the integration boundary. Use a
coherent committed dependency through `lf` if needed; do not copy its dirty work,
edit that checkout, or freeze the superseded personal-plan implementation here.
Callback transport and routing belong to this Task; local planning and Linear
sync semantics remain with LOO-406. An unavailable dependency is reported with
its exact required API after independent callback work, not replaced by a second
implementation.

## State and transport

- The originating invocation serves a Unix socket and carries it to the registered
  target through OpenSSH remote Unix-socket forwarding. Reuse existing transport
  mechanics where suitable, including explicit forwarding cleanup. There is no
  shared resident service or inbound TCP listener on the laptop.
- A small typed planning request carries operation, selected repository/plan scope,
  established Task identity or selector, mutation input and expected revision as
  needed. A mutation retains its operation/creation ID across lost replies. The
  host resolves the scope and invokes the same public planning operation used
  locally. No raw SQL, arbitrary shell callback or network-mounted SQLite.
- Scope the connection to this invocation and selected planning owner. The target
  retains its own `LF_HOME`; credentials and process authority never become
  planning data. A callback grants no process signal, Workflow move, remote PR
  settlement or Session control. Use the existing command parser, not a remote
  command allowlist or parallel selector parser.
- Route at the planning operation boundary so agent-issued nested commands and
  Flow steps on mini use the same host plan. Descendants need the planning route
  for the invocation's lifetime. The one-shot pushed-code requirement remains
  consumed at initial placement; it must not constrain later local commits.
- A nested dispatch keeps the original planning owner instead of making the
  intermediate worker the new host. A missing or dead inherited route is an
  explicit unavailable result, never permission to fall back to a worker plan.
- Stop serving and cancel the exact forwarding on invocation exit, including
  when a private SSH control master survives. Do not delete or reuse another
  invocation's socket. No cross-version negotiation or compatibility layer.

`TaskSource` currently transfers `PmTaskRecord` into absent target planning and
can seed the target's Project selection. Replace that ownership model in this
same diff. The worker may retain the Task brief and necessary immutable/cached
context for local execution; that is not a second mutable planning authority.
Read projections identify their host and observation age. No whole-plan replica,
second user-facing Task kind or alternative store resolver.

## Identity, placement and preserved work

The host creates identity once using its normal creation API. New worker records
retain that established Task identity instead of independently minting it. The
previous Linear-derived UUID v5 rule no longer substitutes for carrying the host
identity: locally born Tasks need no Linear issue. Prefer existing Task/issue
mappings and a minimal explicit origin association where needed. If two machines
already hold different IDs for the same issue, preserve both and route planning
to the host's established record without renumbering local execution history.
An operation ID identifies one attempted mutation, not a new Task/Run concept.

Preserve fetch-before-decision and the original pushed-code behavior. The host
names uncommitted/unpushed branch or commit errors without committing, pushing or
resetting anything. The worker fetches before branch placement, including branches
without PRs. Existing dirty target files, HEAD, Task/PR identity, Workflow and
Sessions survive rejection. A retained target checkout behind the required
commit reports the branch, commit and `lf sync` in that checkout.

A host-created Task with no code yet has no fabricated required commit or checkout.
Its first remote placement follows the ordinary new-Task branch/base rules and
records execution on the worker. Host planning is visible immediately regardless
of whether placement later succeeds. Do not make planning creation require a
local checkout or an agent. Existing worker Project selection/rotation and
invalidation/removal evidence must not be overwritten to admit the host's plan.

## Failure and recovery

For this slice, no direct worker-to-Linear fallback after host loss. The host owns
its ordinary provider synchronization and reports saved-local/pending-sync
separately from provider confirmation. This is an implementation choice under
Jack's callback selection; it keeps the earlier unresolved fallback out of scope.

- Persist original mutation identity, input and outcome at the host's writer.
  Repeating the same operation after an accepted write loses its reply returns
  the committed result, never a second Task/comment. Serialize an in-flight repeat;
  an absent receipt while the first write can still commit is not non-execution.
- A disconnected planning write reports unavailable or unconfirmed and retains
  authored input for deliberate retry. It cannot report success merely because
  the worker retained text, and it cannot allocate a worker-local replacement Task.
- Reconnect reads the original operation's outcome. Reconcile uncertain writes
  before deliberately submitting anything new. Do not automatically replay old
  edits or retry a turn/Flow. Cached reads keep their age and identify the owner.
- Preserve existing execution outcomes and histories on mini. The callback adds
  no disconnect-survival guarantee, detachment, terminal relay or automatic
  continuation; LOO-414/415 own those mechanisms. Host loss is not Task completion
  or evidence that a provider process exited. Losing a worker does not erase the
  host plan; losing the host still needs backups or later Git publication.

## Delete and integrate

Delete the copied-planning/bootstrap owner that the callback replaces, including
Project selection mutation for remote adoption. Keep one Task resolver and normal
planning operations; do not retain both snapshot-import and callback paths as
competing modes for the same command. Retain code requirements only where they
are still needed for placement. Reuse the host's creation receipts and mappings;
remove remote ID minting that competes with them. If persisted routing needs a
schema change, use one Task draft against the released frontier and preserve all
existing history. No migration is required merely to describe a transport.

Existing account forwarding is transport precedent, not a planning owner.
LOO-413 owns credential login/forwarding. No code, tests, help or config from
herdr/cmux; no shared resident process, hidden CLI arguments, compatibility shim,
automatic turn/Flow retry, Git-plan synchronization, installation or landing.

## Acceptance for the next review

Use CLIs built from this worktree in isolated stores; the installed release is not
a prerequisite. Apply Release's operation-entry lesson: public `--machine` dispatch
must reach the actual recipient and host operation, not only an adoption helper.

1. A blank worker creates a Task through the host. Read it immediately on the
   host with the same identity and content; no checkout, Session or Flow exists
   from creation alone. Repeat the same creation operation after a lost response
   and show one host Task. A separate same-title creation remains distinct.
2. Remote edit/comment/read and an agent-issued nested planning command use the
   host store. The worker's unrelated Task and Project selection stay unchanged.
   Exercise both local-only host planning and a contained connected-Linear host;
   show saved-local/pending-sync without claiming provider acceptance.
3. A remote skill sees pushed implementation. Repeat by issue/local selector
   and machine label/ID: one execution placement and the same checkout. Cover
   host-born IDs and two pre-existing legacy IDs; preserve populated Workflow,
   Session and PR history.
4. Unpushed source and behind/dirty target counterexamples retain files, HEAD,
   IDs and histories. Missing planning transport cannot create a worker plan.
5. Lose a callback response after commit, reconnect, inspect the original outcome
   and demonstrate no duplicate mutation. Disconnect before acceptance, retain
   the authored input and report its real state; no automatic replay.
6. Real loopback SSH proves forwarding and cleanup while a control master remains
   alive, concurrent invocation isolation and nested dispatch to the original host.
   Stubbed SSH is useful focused evidence but cannot replace this proof. Use an
   isolated account/container where host permissions require it.
7. Refresh the PR description and review walkthrough around the final callback
   implementation. Publish #1491 for Jack Heart's next review and stop there.

The current source and five passing demo scenarios establish only the superseded
adoption path. [Demo evidence](remote-task-demo.md), its capture, and
[the old walkthrough](pr-review.html) remain dated evidence at `0cd8e7f14`.
None proves callback behavior. No new source implementation or acceptance is
claimed by this design update.

Check: `git diff --check` and `lf context --skill realign` — pass for this design update; callback build, focused tests and real transport acceptance belong to the new implementation and gate.
