# Assumptions and tensions

- Chose trusted straight-to-main publication for unattended captures. The
  publisher refuses a dirty tree or a non-default branch, then uses
  `lf commit -m ... -p`; install still succeeds when that precondition is not
  met.
- The freshness and no-churn requirements conflict when pixels remain genuinely
  unchanged: advancing `captured_at` creates a metadata-only commit, while not
  advancing it eventually trips the 14-day gate. This draft preserves zero
  no-op commits and lets the age gate block after 14 unchanged days rather than
  silently claim old pixels are new.
- The current local `product` status cannot be captured: its PM snapshot has a
  stale Project Session and the Wave is not served. The install hook therefore
  exercises the intended skip path until live state is repaired outside this
  change.
- The laptop install hook is the primary trigger. The repo now exposes an
  `lf website-screens` flow, but does not install a launchd backstop: current
  `lf cron` jobs are Wave-scoped and run through a resident, so they are not an
  independent fallback. Pick the always-on capture host before declaring that
  schedule.
