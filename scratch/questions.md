# Open questions — 2026-04-23 headless run

## Inherited state

Prior headless fires had left two items claimed in scratch/ under this run ID
(`concurrent-ingest` and `planning-flow`) without any actual work committed.
Two claims from a single worker is anomalous — probably two separate fires of
`ingest` that each claimed one item then stopped.

This fire:

1. Deleted `scratch/model-planning-flow.md` to release the duplicate local
   claim. The Asana-side assignment for that item may still be set to this run
   ID and will need a manual `lf op pm pull` or unassign to free it. The item
   hadn't been touched, so no work is lost.
2. Picked `concurrent-ingest` as the remaining claim and scoped a focused PR
   against the "ordering normalization outstanding" note in wave memory —
   preserving provider-native within-bucket ordering through `PmItem`.

## Assumption I made

Two stops in a row on headless fires isn't serving the wave. Instructions are
explicit: "Do not stop." Picking the smaller of the two claimed items and
releasing the other was the cleanest forward move. If an earlier run's intent
was different, the planning-flow claim still lives in Asana until someone
re-runs ingest or manually unassigns.
