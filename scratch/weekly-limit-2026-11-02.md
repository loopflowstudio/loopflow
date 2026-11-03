# Weekly-limit failure evidence — LOO-714

November 2 evidence for Jack Heart's recovery outcome. Source: main checkout
scratch/desktop-exit-2026-11-02.md, Chain C. Complete copied record, timestamps,
process IDs and proposed alternatives: `04bdec77e3:scratch/weekly-limit-2026-11-02.md`.
No live history/account repair was made; proposed remedies remain design input.

The first 5whys run (Session `session_8fa0bcae…`, Process `5bf2baa1…`, PID 85481)
started on Claude `primary`, provider thread `edd6adc7…`. At 22:01:03Z, event
2829383 reported `provider_rate_limited: rejected: seven_day_overage_included`;
primary entered cooldown until November 4 at 12:00:05Z. Text classification missed
the subscription code, so transient retry kept the thread (`resumed=true`,
`account_failover=false`). Routing skipped primary, chose loopflow and recorded
`edd6adc7… → loopflow` before retry admission rejected the cross-account resume.
The old thread never ran there. The retry invariant was correct; router and retry
had made different account-change decisions.

Seven runs failed this way at 22:00:25–22:01:41Z. Later runs found neither managed
account eligible. Original thread/account attribution may now be wrong and needs
evidence-based inspection in PR2; no historical rewrite is authorized by this note.

Required proof: fake Claude on A emits the weekly rejection; continuation succeeds
on B with a fresh provider thread, the same Loopflow Session/work, and the old
thread still attributed to A. Cover all-accounts exhaustion, reset waiting and both
interactive Session and headless Graph recovery. Routing/retry must share the
actual selection decision, not independent text inference. Exact mechanism and
continuity context remain PR2 design; preserve the retry invariant.
