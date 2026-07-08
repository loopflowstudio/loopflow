# Self-running

The outfit runs itself: installs, releases, hosts, credentials — the
machinery around code stays fresh and self-healing indefinitely, and the
measure of all of it is that no agent ever hands a human a setup step.

## KRs

- Nightly verification and weekly release complete for consecutive cycles
  with no manual repair (Linear 6092ca8); billing and spend stay bounded,
  visible, unsurprising.
- Hosts stay fresh and failures surface as TASKS, not silent drift — the
  sync-skills --global failure class (found 2026-07-08 by log audit) never
  recurs unnoticed; credential expiries (Linear, GitHub, vendor) pre-empt
  instead of blocking runs.
- The done-bar: avoidable human-in-the-loop steps found in agent runs fall
  to 0 for one full week.
- The exec-door story completes: lfq folds into lf or is re-chartered
  (coordinate with datamodel's one-system bet).
