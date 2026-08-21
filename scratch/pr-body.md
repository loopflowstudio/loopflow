## Try it!

```bash
cargo test -p loopflow --lib resident_records_an_invalid_lifecycle_once_without_retrying
cargo test -p loopflow --lib lost_body_parks_an_invalid_lifecycle_without_relaunching
cargo test -p loopflow --lib repaired_persisted_task_flows_validate_through_the_generic_path
```

The first proof runs two resident ticks and observes one resumable failure event,
zero Runs, and one unchanged PR. The second proves the lost-body path stops at
the same boundary. The third verifies that the released historical migration
still repairs the recorded LOO-167, LOO-193, and LOO-195 shapes from `task` to
`slice`.

Before: every Project supervision tick could attempt the same invalid lifecycle
recovery again. After: the first tick writes one actionable receipt; later ticks
write no event, reserve no Run, create no PR, and emit no repeated warning.

## Intent

Make unavailable Task lifecycle flows converge durably. Resident supervision
now validates the persisted plan before PR reconciliation or relaunch, records
the exact refusal once, and parks until the named flow becomes valid again.

Linear Task: [LOO-207](https://linear.app/loopflow/issue/LOO-207/make-managed-tasks-survive-missing-lifecycle-flows)

## Assumptions

- One active Project supervisor owns each Task recovery loop.
- The v0.12.5 `repair_legacy_task_flow` migration remains the sole historical
  `task → slice` data repair.
- Current Task lifecycle selections are immutable; unknown invalid pins are
  restored in place or the Task is abandoned and replaced.

## Key decisions

- Reuse the launch validator instead of creating resident-only flow rules.
- Persist one resumable Task failure so status and supervision share durable
  evidence.
- Compare the exact latest error and warn only when first recording it.
- Stop before Run reservation, PR rotation, or provider launch.

## Not included

- Compatibility flow aliases or runtime flow-name migration.
- Automatic worktree reconstruction.
- Changes to Project or Wave lifecycle recovery.
