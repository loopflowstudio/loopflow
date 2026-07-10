# PM table output — validation

Design folded into `wave/infrastructure/MEMORY.md` (Shipped). Verify the change:

## Done when

- `lf pm show` prints one task per physical line under stable headers; long
  titles cannot collide with project, assignee, or ID fields.
- Open tasks appear before done tasks, preserving Linear rank within each status.
- Every task remains readable as a single deterministic record with a full ID.
- `lf pm show --json` output is unchanged for machine consumers.
