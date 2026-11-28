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

## Asking for a decision

Describe what each choice changes for someone using the product. When asking
only some of a review page's questions in chat, label them with their page
numbers and option letters (`Q3b`, `Q7a`) so a short reply works in both places.
