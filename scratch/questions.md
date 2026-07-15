# Assumptions

- “Stable Wave surface” is interpreted as stabilizing the selected Wave's read
  contract and refresh behavior before adding new conduct interactions. The
  branch started clean with no inherited design; the recent trajectory surface
  was explicitly reverted, so this PR does not infer that interaction back in.
- `lf context` could not recover additional run context because the installed
  `lf` knows migrations through `0.11.007` while the local ledger is already at
  `0.11.008`. The code and committed product history were sufficient to proceed.
