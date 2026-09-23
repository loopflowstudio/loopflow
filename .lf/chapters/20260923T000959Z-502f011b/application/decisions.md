# Application decisions — 2026-09-23

The accepted packet and final parent instruction authorize PM changes and precise
Work retirement. They authorize a local checkpoint, with no remote publication.

- Live provider refreshes matched every frozen Project and Task before mutation.
- `lf pm doctor` enumerated the current provider Initiatives. Engbot is neither a
  linked local Wave nor an unlinked provider Initiative in that enumeration. Its
  focused status has no Project, Task, Run, or resident; its placement is disabled.
  There is no bound provider object to archive. Retire only its exact registered
  Wave Work, preserving registration and history.
- `lf work abandon` is the supported accepted lifecycle operation. It changes the
  exact Work's Ready state to Abandoned, without deleting artifacts or signaling
  processes. It does not populate the separate promotion `retired_at` metadata;
  verification must inspect Work state, not misreport a promotion retirement.
- The 119 residual PM-complete Ready Tasks have unchanged IDs, state, references,
  and PR histories. None has observed dirty or authored local progress. Missing
  checkouts stay missing. Terminal historical Tasks are carried without reopening.
- LOO-274 closes through `lf pm task done`, whose public API has no cancellation
  state. Its authoritative directive explicitly says retired, not shipped or
  achieved, and preserves the rejected fleet scope and history.
- The existing wave-agents branch has no Task association in the complete fresh
  PM and Work projections; `lf task status wave-agents` reports no Task. Create
  one tracking issue for that existing change without preparing a checkout,
  starting/restarting a controller, or creating a second writer. Its directive
  preserves the branch and requires serial-PR association before any launch.
- Parking and serial allocation live in authoritative Task directives and this
  accepted chapter. They do not introduce a new PM state or runtime concurrency
  mechanism. No controller is launched as part of chapter application.
- The three retained Wave charters are edited here. Their deployed canonical
  checkout remains unchanged until the human authorizes publication and landing.
  This is the required ordinary repository boundary, not a reason to edit another
  checkout or publish without authority.

Review findings fixed before sealing: the obsolete task-loop-trust contract would have relabeled settlement as external progress; it is removed from active contracts and archived unchanged. LOO-185 retains identity, idempotence, permission and secret-handling proof while dropping the mandatory two-Home rollout. LOO-274 closure explicitly records retirement, preventing a PM-complete flag from being mistaken for shipment. Desktop budgets remain unproven evidence work.

Name-reference reconciliation found a historical lifecycle projection limit: `lf project status architecture-minimalism` cannot resolve the newly renamed slug, while the stable Project UUID resolves the same existing Work and its old captured execution-plan label. `lf pm show`/`lf status` expose the accepted current provider plan. All chapter operational references therefore use stable Project or Work IDs (project-references.json); historical captured plans are preserved, not relabeled or restarted. The probe is recorded as an observed command failure, not a failed PM rename. No direct store write or controller launch is used to repaint historical execution context.
