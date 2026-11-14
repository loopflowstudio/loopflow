# Desktop database mismatch — November 2 evidence

This is a separate incident from LOO-715's WebKit crash. At 10:25 PST,
LaunchServices launched a benchmark staging copy, 0.14.19, instead of the installed
0.15.1 app. That copy could not read the 0.15.x store. Unregistering the stale
copy and missing registrations restored the installed app without touching the
store. The originating launch request remains unidentified.

LOO-620 / #1798 delivered launch identity; LOO-693 / #1797 delivered store-skew
handling. Their current code/history own the repairs. The November 2 report's
proposals are historical, not new direction for this Task.

The complete original uncommitted report was preserved before curation at
`~/Library/Logs/Loopflow/desktop-exit-minidumps-2026-11-02/source-notes-20261114/desktop-database-mismatch-2026-11-02.md`.
SHA-256: `c9d97d39ec51ac64baa0ad64e0bc74a1db3f311717333c6abbd16b25d2732292`.
It is also embedded verbatim in this checkout's
`.lf/prompts/20261114-083755-2da6c7de-cc05-48f9-84ba-a99e4f5c66d3-2Q1qU9-summary.md`.
