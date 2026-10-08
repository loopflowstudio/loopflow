# Product decisions still open

The [plan](compare-cmux-s-command-line.md) owns implementation; [findings](findings.md)
own evidence. Jack Heart has already selected one shared Work tree, subtree
delegation, one repository window across machines, macOS-only view control and
one PR. Those are not questions to re-derive. Slice 1 is implemented locally; slices 2–5 still depend on these decisions.

## Q1 — Shared source availability and writes (blocks slice 2)

Where does the authoritative Work tree live, where do planning writes go, and
what remains possible when that source cannot be reached?

Two mechanisms still fit the accepted one-source direction but have different
product consequences:

- A designated planning machine owns the existing store; other machines address
  it for planning reads/writes, keeping caches observational. This requires the
  source to be reachable for writes and an explicit recovery/backup policy.
- A durable published plan is authoritative, and execution machines consume its
  exact identities. This requires a publication/write-conflict protocol and a
  decision about local-only/private plans; it cannot silently publish to the code
  remote. Jack's earlier Git leaning is not approval of this protocol.

These are alternatives, not selected implementations. LOO-406 supplies local
planning operations; its current scope deliberately leaves synchronization out.
Q1 must include existing connected plans and repositories with no Tasks.

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
