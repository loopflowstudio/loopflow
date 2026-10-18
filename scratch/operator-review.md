# LOO-505 implementation review notes

2026-10-17 — Resolved against the completed ownership change.

`ops/project.rs::set_workflow` delegates validation to
`store/sqlite/project_content.rs::select_wave_workflow`. That owner parses the
selected definition inside its transaction before saving either selection or
source. The focused regression
`stored_workflows_use_the_same_owner_with_and_without_linear_mapping` passed:
malformed definitions and invalid names preserve the prior default and source,
including after reopening the store. No duplicate command-boundary parser is
needed.

The follow-up review found that standalone creation previews still rendered
“Project unavailable” and `global_commands.rs` retained implicit-inbox and
recorded-work refusal assertions. Creation now distinguishes known absence
from failed selection; the preview tests cover ambient bypass, attach/detach
with a checkout, remote destination placement, and unchanged storage. No
remaining finding from this operator review.
