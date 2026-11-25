---
extends: builtin
---
## A step that keeps failing

**The second failure for one cause is yours to repair.** Once a step fails twice
for the same reason, stop reporting retries: read its log, wait until no run is
live, and make the routine fix. For an exhausted quota, run
`lf task run <issue> -a claude` (or whichever agent has quota) so remaining
steps run there, then say what you ran and the next stop.

## Review answers that ask for building

**Answers that change the code go back to `implement`.** From `ship-review`,
plain `lf task run <issue>` continues to delivery. When answers name work to
build, steer them to the Task and run `lf task run <issue> --from implement`.

## Asking for a decision

**Frame choices in product behavior, not internals.** When a conflict or design
clash needs a call, describe what changes for someone using the product under
each option; leave table names, flags, migration marks and SHAs out.
