# Product decisions still open

The [plan](compare-cmux-s-command-line.md) owns implementation; [findings](findings.md)
own evidence. Jack Heart has already selected one shared Work tree, subtree
delegation, one repository window across machines, macOS-only view control and
one PR. Those are not questions to re-derive. Slice 1 is implemented locally; slices 2–5 remain. The former shared-source selection blocker is resolved.

## Q1 — Selected: local operations and custom Git-ref Task sync (October 8)

Jack Heart superseded the designated-planning-machine experiment: everything in
an invocation runs against the execution Machine's ordinary local store. A custom
Git planning ref synchronizes Task planning between machines. LOO-406 owns the
common local writer/Linear sync; LOO-412 owns exchange/merge and active sync scope.
No planning callbacks or centralized Workflow writes remain in this plan.

`lf task run` stays local, including Workflow take-up, edge selection and arrival.
Only portable planning changes synchronize. Imported completion changes planning
presentation, never the receiving Machine's Workflow, Processes or checkout.
Unavailable Git retains local work and visible pending sync. Q1 is resolved;
protocol/hosting proof is LOO-412 implementation work, not a renewed transport vote.

## Q2 — Changing delegation (needed in slice 2)

Proposed behavior: nearest explicit ancestor supplies the assignment, narrower
assignments override it, and edits govern future execution. Existing running
work/checkouts keep their recorded locations until explicitly transferred. This
avoids implied live migration; exact reassignment UX remains unaccepted.
Historical provenance is an implementation constraint: preserve it as unknown
where the existing rows cannot distinguish explicit from copied assignments.

## Q3 — Input into an occupied Session composer (needed in slice 4)

Choose how targeted input coexists with an existing unsent draft under LOO-387's
preparation contract. Silent replacement is excluded. Literal insertion and
separate Enter remain required but do not by themselves protect the draft.

Transport, API spelling, schema field names and queue implementation are ordinary
implementation choices. They are not additional product-approval checkpoints.
