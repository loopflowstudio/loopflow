# Open questions / blockers

Decisions, the perf audit, the daemon-gate diagnosis, and the compress-pass
findings from this branch are folded into `wave/concerto/MEMORY.md` and the
`projects/*.md` files. What remains genuinely open:

## Blocked

- **Linear roadmap not reconciled.** `lf op pm show` fails: "Stored linear token
  has expired. Run `lf op auth linear` again." The task-tier roadmap could not be
  updated this run. Surfaced work is recorded in MEMORY instead: the runs-ledger
  slice, the `WaveService` collapse, and the `lf wave stop` product gap.

## Needs a human decision

- **`WaveService` facade collapse.** ~600 lines of retired-lfd-HTTP methods that
  `throw unsupported(...)`, still called behind live UI actions (stop/delete/
  land/next/addTrigger/combinePRs, session create/attach/cancel). Deleting it is
  a behavior change tied to the session-lifecycle + wave-conducting projects, not
  a compress edit. Its dict-based `parse*FromJSON` is a second wire mirror that
  backs fixture/contract tests — consolidating onto RegistryQuery's Codable path
  is a real cross-test refactor. Details in MEMORY ("Swift data path — Known debt").

## Assumption made this run

- Wrote `wave/concerto/MEMORY.md` directly: the wave has no live server
  (`lf memory update` refuses, offline queuing unimplemented), and update-wave is
  the sanctioned owner of `wave/concerto/`. No live journaling server to clobber.
