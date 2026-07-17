# Assumptions and tensions

Resolved by the 2026-07-17 revision (scratch/living-website.md):

- Straight-to-main publication stands. The publisher refuses a dirty tree or a
  non-default branch, then uses `lf commit -m ... -p`; install still succeeds
  when that precondition is not met.
- The freshness/no-churn tension is dissolved by splitting the deploy gate:
  structural failures block, staleness warns loudly and ships. Zero no-op
  commits remain possible without the age check blocking unrelated docs or
  website deploys.
- The busted-live-state note is superseded: the liveness bar is a served Wave
  only. Red or failed task states are honest and publishable, so the current
  imperfect `product` state is a valid capture subject once the Wave is served.

Parked with the revised bar:

- Backstop capture host (laptop launchd vs mini-heart) is deferred along with
  Done When 2's four-unattended-weeks streak. This round's only triggers are
  the install hook and a hand-run
  `uv run python scripts/refresh_website_screens.py --publish`.
