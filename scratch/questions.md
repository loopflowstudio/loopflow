# Open questions — W2-169

## Deferred within Slice 1 (non-blocking)
- A cross-command side-effect-free boundary test over `lf status`, `lf roadmap`,
  `lf doctor`, `lf project/task status`, `lf diff`. Exploration confirmed none
  call `sync_main` today, so `wt list` was the only live mutation bug. Adding the
  broader test needs a cheap harness for those commands (some want a wave/registry
  setup). Assumption: defer until that harness is cheap; the `wt list` boundary
  test already guards the surface where `sync_main` actually lives.

## Carried forward (later slices)
- Convergence-tick cap distinct from the generic 8-pass / 2-hour task defaults
  (Slice 3) — pick a conservative bounded default, tune from one real project
  loop's dogfood data (per MEMORY's "project-loop caps" open fork).
