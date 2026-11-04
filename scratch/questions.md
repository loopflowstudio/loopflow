# LOO-717 — Accepted implementation scope

Jack Heart approved building this design on November 3, 2026: “ok. lets build it!”
Implementation and review publication are approved through interactive demo;
shipping, merging and installation are not.
The current design is scratch/loo-717-shell-lifetimes.md; historical exploration
and the simulated review do not override the accepted scope below.

## Accepted

- Host-native ownership: Rex owns native layouts/shells; Desktop owns its own.
  lf treats both hosts equally for Session retention. No mirrored Rex split tree,
  mandatory shell relay or optional attachment to live lf ordinary shells in Rex.
- Local and remote Desktop shells use the Task's recorded Machine/checkout.
  Remote failure never produces a local prompt. Reconnect reuses live execution;
  New Shell is deliberate and does not replay arbitrary old commands.
- Close a shell ends it and its ordinary jobs. Hide/Task switching/app exit retain
  ordinary shell execution, subject to completed-Task cleanup. Undo cannot revive
  an ended process. A server running inside an ordinary shell is not an agent turn.
- Host retention keeps a Session alive. Each host chooses release policy; lf does
  not interpret tab selection. Desktop releases completed-Session retention on
  departure rather than prioritizing keeping old completed Sessions resident.
- Ordinary eligible idle agent ending uses LOO-709's two-minute rule. For completed
  and released Tasks, checkout and shells remain until fifteen minutes total
  (two plus thirteen). No Task-membership exemption for idle agents.
- Requested active work must finish even if the Task is done and its window is
  closed. Preserve LOO-709's draft/pending-request protections. Explicit Close
  can end eligible live execution once work settles without a new absence grace.
- Completed-Task cleanup discards uncommitted tracked edits and untracked files in
  the eligible Task checkout. Dirty files do not veto cleanup. Existing handling
  of changed/unmerged committed refs, primary/persistent checkouts and unrelated
  resources remains; do not widen deletion to other work.
- Retain Task and conversation history. Ordinary resume during retained-checkout
  grace remains. Automatic restoration of a deleted checkout is outside scope.

## Implementation decisions delegated to the build

Use one real host retention/release contract, existing attachment/process evidence
and one cleanup authority. Prove behavior on clean release, client crash, remote
loss and multiple retained occurrences; a saved layout is not live retention.
A failed host read is not confirmed closure. Reuse existing owners and eliminate
replaced code rather than adding another lifecycle stack.

For the workspace deadline, use fifteen minutes from eligible completed-and-away
entry; host reuse cancels it and later release starts a fresh interval. Active
work prevents execution of cleanup; it does not require another fifteen minutes
once an already-due cleanup becomes safe. This is an implementation interpretation
of Jack's wait-for-final-work direction, not an additional timing choice from Jack.
Keep the existing agent idle clock/protections distinct. Record/test exact races.

LOO-498 owns the Rex-specific adapter and workspace features and remains independently owned. The shared contract and Desktop path are implemented here; concrete API needs
were shared in the coordination note. Rex end-to-end proof remains outstanding.

Main through `ac8e088f5f`, including LOO-709, LOO-711 and native status restoration
across reconnects, is integrated. LOO-714 routing remains an overlap constraint.
The Task uses one migration. Remaining
implementation and proof are tracked in `loo-717-shell-lifetimes.md`.

## Build assumptions — November 3

A lost retained-host lease is unknown. Cleanup needs fresh release or positive
evidence that the exact retaining owner exited, per the accepted November 4 choice. Desktop releases on departure, including unfinished Tasks; ordinary
shell execution stays alive and an eligible idle agent uses its existing grace.
Host observations carry increasing revisions so an old release cannot overwrite
a newer return. Reusing an occurrence across restarts requires persisted revisions;
Desktop uses a fresh occurrence per app process and an in-memory counter per retained
workspace. Native Rex integration remains owned by LOO-498.

## Earlier headless concept-review interpretation — November 3

Clean Desktop quit is departure, not explicit Close: it preserves ordinary
shells and the completed Task's fifteen-minute grace. This follows Jack Heart's
accepted quit/Close distinction; it is an engineering interpretation, not new
approval. The implementation separates release policy from native-resource
settlement in the existing host observation, so acknowledged quit can finish
cleanup without reopening Desktop. Failed reporting remains unknown. Exact
direction and counterexamples are preserved at
`e7fd5db44f:scratch/loo-717-shell-lifetimes.md`, **Concept review — November 3**.
No unresolved product choice was found.

Build choice: quit has one five-second reporting budget across all destinations.
Desktop fences window Close and heartbeats before detaching surfaces; failed or
canceled writes provide no acknowledgement. A prior explicit Close keeps its
policy. This bounds departure only, not requested work or the cleanup deadline.

## Interactive concept review — accepted November 4

Jack Heart requested another concept review at demo. Before the November 4 repair,
an expired retained-host lease held completed-Task shells and checkout indefinitely;
even a dead Desktop instance needed a fresh host report. Eligible idle agent ending
was already independent of that workspace hold.

Jack Heart selected **1a** ("1a i guess") on November 4: positive evidence that the exact retaining host
instance exited releases its hold and starts ordinary absence cleanup without
reopening the app. Requested work and independent native/destination settlement
still precede deletion. Network loss or lease expiry alone remains unknown;
Rex client disappearance does not establish its retaining server's death.

The choice is resolved; the confirmed-exit path is implemented with focused local,
real SSH and headless Swift evidence recorded in the working design.
The working design preserves the contract and evidence limits; the earlier review
and counterexamples remain at `e7fd5db44f:scratch/loo-717-shell-lifetimes.md`.
The review granted no shipping approval.

## Implementation continuation — November 4

Jack Heart requested "build it" after accepting 1a. Implement and prove confirmed
retaining-host exit recovery, then republish PR #1811 for interactive demo. Keep
the existing feature Graph and Task checkout. No Auto-Ship, merge or installation.
Reuse the accepted design's protections and independent native/destination
settlement. Share concrete adapter changes with LOO-498 without launching its work.

Implementation choice — November 4: use the existing retirement executor and OS
process reader. A host owner names Machine, PID, kernel start time and whether
native resources die with that process. Remote verification uses an already
registered route back to that Machine; absent routes/network failure stay unknown.
No new watcher daemon or reverse connection setup. Desktop process instances get
independent occurrences; old-instance exit cannot release a newer instance.
