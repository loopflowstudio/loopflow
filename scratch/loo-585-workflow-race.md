# LOO-585: local_planning tests raced their detached Graph

Symptom: CI/merge-group panics at local_planning.rs:3823 (`waveless_task_lifecycle`) and :3919
(`workflow_defaults_follow_task_wave_and_repository`) comparing Workflow JSON whose `history` differed.

Cause: #1675 made bare `lf task run` return once the Graph registers; the arrival is written later
by the detached owner. These tests (and `two_people`'s takeover run) still used bare `task run`
and then read settled state. `-b` still waits for the Graph and its arrival.

Repair: tests that read state after a run now use `lf -b task run`. Tests that exercise detaching
(task_graph_launch_tests) or refusals and no-Graph edges are unchanged.

Verification (12-step Graph probe to widen the window, CPU saturated with `yes` per core, 5 rounds × 2 tests):
- before (bare `task run`): 10/10 failed, same lines 3823/3919 as CI
- after (`-b`): 0/10 failed
- unmodified tests, loaded, before fix: 0/20 failed locally (window too narrow to hit without the probe)
- `cargo test --lib local_planning`: 39 passed, 1 failed, `store::sqlite::children::…task_labels_and_generated_names…`
  ("capture does not belong to a recorded Session in this Machine"). Deterministic here, untouched by this diff,
  green on main CI: looks local to this machine, left to CI.
- `cargo test --test two_people`: ok. clippy -D warnings: ok.

Note: PR #1679's ci-fix `a0ef70f1e7` polls for the arrival in `waveless_task_lifecycle`; whichever lands
second will conflict at that hunk. Keep the `-b` form.
