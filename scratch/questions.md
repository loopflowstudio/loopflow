# Open questions — W2-169

## Deferred within Slice 1 (non-blocking)
- A cross-command side-effect-free boundary test over `lf status`, `lf roadmap`,
  `lf doctor`, `lf project/task status`, `lf diff`. Exploration confirmed none
  call `sync_main` today, so `wt list` was the only live mutation bug. Adding the
  broader test needs a cheap harness for those commands (some want a wave/registry
  setup). Assumption: defer until that harness is cheap; the `wt list` boundary
  test already guards the surface where `sync_main` actually lives.

## Cross-Task coordination (W2-171)
- W2-171 owns the landing/completion slice: out-of-band merged `TaskPr`
  reconciliation + the `.lf/tmp/scratch-stash` path. W2-169 does not edit
  `reconcile_task_pr_with_authority`'s merge→settle/complete transition. The
  stable-identity/append-only fix for the reused-branch overwrite lives at commit
  `bcb11c6cc` for W2-171 to cherry-pick. W2-169's read-side PR freshness (Slice 2)
  consumes that seam; sequence the shared edit after W2-171 lands it.

## Carried forward (later slices)
- Convergence-tick cap distinct from the generic 8-pass / 2-hour task defaults
  (Slice 3) — pick a conservative bounded default, tune from one real project
  loop's dogfood data (per MEMORY's "project-loop caps" open fork).
