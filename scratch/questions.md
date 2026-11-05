# LOO-717 — Accepted implementation scope

## Current precedence — November 5

Jack Heart clarified that only an open LF viewer of a Task Session retains the
checkout; a random shell in its directory does not. The LF viewer process lifetime
is the boundary. A surviving native tab after viewer exit is not an extra hold.
No LF persistence of external host/workspace/occurrence identities, and no LF
closure of Rex shells. These supersede older host occurrence/lease/native-settlement
assumptions below. The rewritten loo-717-shell-lifetimes.md is authoritative.
Jack requested a new pursue run after this design update, through review publication
only; no ship, merge, installation or live-store migration. No open product choice.


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
- An open LF Session viewer keeps a Session retained (Nov 5 reading of the
  original host-retention item); lf does not interpret tab selection. Desktop
  releases completed-Session retention on departure.
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

For the workspace deadline, use fifteen minutes from eligible completed-and-away
entry; a returning viewer cancels it and later release starts a fresh interval.
Active work prevents execution of cleanup; it does not require another fifteen
minutes once an already-due cleanup becomes safe. This is an implementation
interpretation of Jack's wait-for-final-work direction, not an additional timing
choice from Jack. Keep the existing agent idle clock/protections distinct.

LOO-498 owns the Rex adapter and native layouts; Rex end-to-end proof remains
for demo. Main through `ac8e088f5f` (LOO-709, LOO-711, native status restoration)
is integrated; LOO-714 routing remains an overlap constraint. One migration.

## Superseded host-occurrence notes

The November 3–4 lease, revision, quit-budget and confirmed-host-exit build notes,
including Jack Heart's November 4 choice of **1a**, are at
`b81df60579:scratch/questions.md`. 1a's outcome (a dead Desktop releases without
relaunch) now follows from its viewers' connections ending.

## Current Rex boundary — November 4 direction, reconciled November 5

Jack Heart's accepted direction in comment `04388b59-a9ab-434c-a1d2-c120ec38b7e9`
supersedes every earlier Rex away/timed-close/native-settlement instruction above.
LF automatically cleans up only Loopflow Desktop-owned shells and must not close
Rex shells. A Rex terminal retained for an LF Task Session keeps the Task retained
even when hidden or unfocused. Confirmed removal of the last relevant terminal
releases that hold, subject to ordinary conditions and other holds. Unavailable
inventory remains unknown. Rex owns layout mutations and restoration; LF layouts
are one-way input. LOO-498 owns its native adapter.

## Build decisions — November 5 pursue (reversible, engineering)

- Viewer hold lives on the serving owner's Process (`processes.viewer_hold`),
  added in the Task's one migration; `task_host_occurrences` is gone.
- SSH-ness comes from the relay's `SSH_CONNECTION`. A hangup under SSH (Rex tab
  closed on a remote viewer, Desktop quit, sleep/network loss) stays unknown
  until any fresh attach to that owner or a resumed owner replaces it. Whether
  remote Desktop quit should instead send a deliberate leave is an open choice
  for Jack in loo-717-shell-lifetimes.md. Desktop
  departure and window close send Ctrl+] so they release. Cost: a remote viewer
  closed by hangup keeps a completed checkout until the Session is viewed again.
- A dead owner that never recorded its viewers reads as unknown (no reaping).
- Viewed Desktop shells count as LF holds; ordinary shells never do.
- Desktop window Close no longer skips the fifteen-minute grace (no host
  channel reports explicit Close); shell Close keeps its explicit behavior.
- The Desktop retirement banner went with `lf task presence`; the pending
  reason stays in `task_retirements.reason` and `logs/workspace-retirement.log`.
  A Task status surface for it would be a separate change.
- Wave memory (`.lf/wave/product/MEMORY.md`, LOO-717 section) now records the
  Session-viewer hold (realign, Nov 5).

Headless proof does not cover the real Ghostty detach timing (500 ms before
freeing the surface) or Rex end to end; both belong to demo.
