# PM table output

## Problem

`lf pm show` uses fixed-width fields inside project groups. Long task titles run
into their metadata, and unassigned tasks repeat the same project marker twice.

## Approach

- Render one task per physical line under stable column headers.
- Measure columns from visible content, sharing the same padding primitive with
  `lf wt list`.
- Put open tasks before done tasks while preserving Linear rank within each
  status.
- Keep full task IDs and offer `--json` for strict machine consumers.

## Done when

- Long titles cannot collide with project, assignee, or ID fields.
- Every task remains readable as a single deterministic record.
- `lf pm show --json` is unchanged.
