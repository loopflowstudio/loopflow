# Release Stability

Shipping loopflow is routine. Main stays green, releases happen on cadence, and
failures stop the line with clear evidence instead of becoming background
noise.

## KRs

- Four consecutive weekly releases and fourteen consecutive nightly
  verifications complete with zero manual repair.
- Main stays green for a month; any red is met by a task within a day and
  never ages into background noise.
- Host and cron failures surface as actionable work before a human notices
  them — one month with zero silently-drifting hosts (the sync-skills
  --global class of failure never recurs unfound).
- Billing and agent spend stay bounded, visible, and unsurprising across a
  month: no invoice line requires archaeology.
