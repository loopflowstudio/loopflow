# LOO-629 questions and assumptions

- Proof boundary: the cron's real command (`lf --wave infrastructure --batch graph release-run`
  → `repo release run patch`) has no dry run, and running it against the live repository would
  cut v0.14.17, which the Task forbids. The proof is the scheduled-release fixture, which drives
  the real `lf` binary through `run_cron` with a seeded superseded selection. The live Machine
  picks up the fix only once an lf containing it is installed at the cron's `lf_path`
  (`~/.lf-machine/install/gates/1/lf`).
- Settlement choice: the superseded v0.14.13 selection is recorded as a candidate replacement
  (`superseded-release-selection` evidence), and the run then releases from current main. All
  outstanding opportunities since 2026-10-21 settle with that run's outcome, so the obligation
  isn't left open. The alternative was to settle them as "Published v0.14.13", which can't be
  verified: the publisher's `reconcile` refuses any tag that isn't the latest release.
