# LOO-717 — Accepted implementation scope

Jack Heart approved building this design on November 3, 2026: “ok. lets build it!”
Continue the existing feature Graph from explore through implementation and
publication to the interactive demo. No shipping, merging or installation approval.
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

LOO-498 owns the Rex-specific adapter and workspace features and is independently
in design discussion. Do not resume or alter its work. Define and test the shared
contract plus Desktop path here, share concrete API needs, and carry any unavailable
Rex end-to-end proof explicitly to demo rather than inventing host evidence.

Sync current main before changing overlapping transport code; LOO-709 merged after
this checkout's base. Preserve LOO-714 routing and LOO-711 UI changes. Use the existing
Task checkout and one migration if needed. No competing implementation process.

Demo disconnected/live versus ended/fresh shells, process cleanup, active-work
protection and two-stage completed-Task cleanup with uncommitted files. Real remote
proof and process/PTY/socket inventories complement the automated policy checks.

## Build assumptions — November 3

A lost retained-host lease is unknown and keeps cleanup pending until a fresh
observation. Desktop releases on departure, including unfinished Tasks; ordinary
shell execution stays alive and an eligible idle agent uses its existing grace.
Host observations carry increasing persisted revisions so an old release cannot
overwrite a newer return. Native Rex integration remains owned by LOO-498.

The compress step's explicit context-budget curation shortened historical Product
memory without changing accepted direction. Earlier implementation correctly left
it unchanged; curation is limited to the required size reduction.
