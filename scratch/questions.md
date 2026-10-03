# Open design questions

2026-10-02 — Jack authorized pursue through demo. Scratch stays local and survives
publication. Use direct resident publication; no separate export worktree is needed.

- Scheduled maintenance cadence and non-PR memory distribution remain deferred.
- Owning Wave remains unresolved; execute in the supplied checkout without filing
  a synthetic Task or inventing placement.
- Implementation may choose the smallest safe upkeep boundary, preserving live
  writers and dirty files; document its behavior and recovery in the owning skills.

2026-10-02 — Implementation choices: upkeep is explicit `lf sync`, never an
opening-message fetch/merge. Scope placement uses stable Wave identity. A
`lf wt create <name> --resident` path also marks an existing independent checkout
for scratch-preserving document delivery; no Task or Wave is inferred.

2026-10-02 realignment: scheduled installation now emits `lf home install`.
Checkout restoration required collision recovery for resolver-created scratch
files; the bounded repair preserves both versions rather than overwriting either.
This follows Jack's existing scratch-preservation constraint.

2026-10-02 bounded repair: preserve resolver-created collisions as unused
`<path>.lf-sync-N` siblings, reserving names already in the stash. Restore original
paths and the caller’s index afterward; reconcile note contents explicitly. No
automatic text concatenation or new recovery store.
