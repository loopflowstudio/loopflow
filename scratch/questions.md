# Open assumptions

- `lf task "<seed>"` takes free text; `--flow` loops any flow. No tracker in
  the path: the open runs with a task flow are the wave's open tasks
  (`lf runs`); the merged PR is the record of done. `lf op pm` remains for
  waves that still want Linear, but nothing in the flowloop requires it.
- The loop file is `scratch/loop.yaml`, consumed every boundary; the runner
  injects the how-to-terminate instruction into every pass seed.
- Executive calls awaiting review, not blocking: `HEARTBEAT_IDLE` = 4h for
  pass-based waves; recheck predicates run under `sh -c` in the worktree.
- The wire rename shipped with NO journal compat: pre-rename `journal.jsonl`
  files no longer fold; existing wave journals reset on first boot after
  deploy — accepted, single-user.
- Project tier: KR set lives in the project's own doc (`## KRs` checklist),
  not Linear. Driver deleted with the tier collapse; `project-pass` skills
  remain and loop via `lf task "<seed>" --flow project-pass` when wanted.
