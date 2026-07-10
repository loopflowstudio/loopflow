# Open questions — intelligence wave

## 1. Linear is unreadable, so project selection is running blind

`lf pm show --wave intelligence` fails: *"Stored linear token has expired. Run
`lf auth linear` again."* This pass therefore read only the local project files,
never the live tasks, and could file nothing. If a task in Linear contradicts a
KR here, this pass did not see it. **The next wave loop must run `lf auth
linear`**, then file the open work now parked in `MEMORY.md` → "Open work" (the
PR delivery record, the `escalated` event, the evals harvester, `lf wavechat`
steering verbs).

## 2. Weekly cadence is prose, not a wired cron

The `daily` audit is now real: `crons:` in `GOAL.md` runs
`.lf/flows/telemetry-daily.yaml` (`lf doctor`). The `weekly` audit named in
GOAL.md's `## Cron` section has no flow behind it — it is intent for the wave
mind to honor when it loops, not something that fires unattended. Open: author a
`weekly` flow (inspect recent evidence, file one measurement-justified edit), or
keep it as prose the mind executes.

## 3. Is hootro driven through loopflow, or the control arm?

hootro is genuinely dormant (no `.lf/journal`, nothing since 2026-06-19). For
the portfolio eval it is either a repo to adopt or the untouched baseline. Both
are defensible; the evals harvester targets cadenza first regardless, so this is
not blocking.
