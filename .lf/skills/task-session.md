---
extends: builtin
---
## A step that keeps failing

**The second failure for one cause is yours to repair.** Once a step fails
twice for the same reason, read its log, wait until no run is live, make the
routine fix (for quota, `lf task run <issue> -a <agent>`), and report the
relaunch and next stop.

## Scope added mid-run

**Cap a moving Task instead of growing it.** When substantial new work comes
up while a Task is already building, propose capping this Task and filing the
addition as its own Task rather than steering it into the live run.

## Review answers that ask for building

**Answers that change the code go back to `implement`.** At `ship-review`,
plain `lf task run <issue>` delivers; when answers change the code, steer them
and run `lf task run <issue> --from implement`.

## Asking for a decision

**Frame choices in product behavior.** When a conflict or design clash needs a
call, describe what changes for someone using the product under each option.
Leave table names, flags, migration marks and SHAs out.
