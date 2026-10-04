# Open questions and assumptions — LOO-376

- **Selected Session is not restored at launch.** Restoring it would run
  `lf session connect` and resume a provider from saved state without anyone
  asking. The saved Work selection (Wave or Task) is restored; its Sessions are
  one click away. Reversible if Jack wants the conversation reopened too.
- **Cache lives in the Home** (`<Home>/desktop-cache/workspace.json`), not
  `~/Library/Caches`, so an isolated Home gets an isolated cache without a
  second knob. Assumes `lf` tolerates an unknown directory in its Home.
- **Saved Sessions keep their `open` action.** Opening is validated by `lf`;
  move-here, complete and Flow controls wait for this launch's read.
- **UI-test and fixture modes never restore**, including `live`, whose proofs
  must show only what they read.
- **Scope signal for the Wave:** `lf roadmap --all --json` (14.5 s) and
  `lf session list` (12.4 s) on Jack's Home have no owning Task. Candidate
  measure: p95 wall time of those two reads against the real store; cheapest
  producer is the startup runner's `lf` interval table.
- **Task condition is quieted in saved text** (state `unknown`, reason "Shown
  from the last launch…"), the safe default. Reversible choice for Jack: keep
  the last reading under `Updating…` instead. Rows grouped by condition (NOW)
  sit under unknown until the fresh read.
- **The Portfolio window was folded, not deleted.** It now shares one cached
  model per window; whether that window should exist at all is Jack's call.
- **Portfolio Work list now follows the repository filter.** The old private
  model kept the repository it was first built with.
