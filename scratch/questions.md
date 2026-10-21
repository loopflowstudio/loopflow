
## Config skew with released lf (October 21, 2026)

`lf task run LOO-520 pursue` failed at implement: installed lf 0.14.12 read this
branch's `.lf/config.yaml` and rejected `planning.team` ("unknown field `team`,
there are no fields"). Repaired in 7d0c4047b8: the repository keeps the released
`linear: {team}` block and this build reads it as Linear planning's team rather
than refusing it. Open: when, after this lands and releases, to move the file to
`planning.team` and drop the fold.
