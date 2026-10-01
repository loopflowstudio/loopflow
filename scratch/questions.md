# Questions and assumptions — LOO-348

- **Slow-turn metric band.** The baseline lists four peak-input bands. One Wave
  reading needs one number, so `context-slow-turn-minutes` is the p90 at 200k+
  (baseline 48 min, Codex 200–300k). The band is open-ended so 1M-window Claude
  turns land in it. All bands stay in the report.
- **Silent-hours baseline window.** The Task gives 213 h Codex / 2.4 h Claude
  over 1,349 records without a week boundary. The suggested target uses their
  sum as a weekly ceiling; the report's own baseline-week row should replace it.
- **Input per step has no hand baseline.** No target is proposed for it; the
  baseline-week row supplies the number.
- **KRs not applied.** `lf wave update-plan` replaces the whole plan, needs a
  nonempty `flow:` (the current chapter has none) and resolves targets against
  contracts on the Wave's repository, which do not exist before merge. The
  plan fragment is in `performance/context-cost.md` for Jack to apply.
- **Not run on real evidence.** The dev build refuses the installed 0.12.28
  store (no `session_events`). Verified by unit tests and an empty Home only.
- **A reader that writes.** An unfiltered `lf usage --weekly` publishes metric
  observations (idempotent per week). A separate publish command seemed like
  one more thing to schedule; a publish failure never fails the report.
