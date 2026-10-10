# Open assumptions — 2026-10-09

- Proposed defaults (existing repository tick plus hourly full scan, eight checkouts per pass) are
  draft, not accepted decisions. Disk measurements will inform cache budgets.
- How broad should automatic ownership be for legacy taskless checkouts? Draft:
  report unless lf ownership and exact-head disposition are proven; never infer
  it from the directory name or branch author alone.
- Jack Heart classified Etude experiment output as disposable. Its expiry,
  machine-wide budget and explicit retention mechanism remain to be designed
  alongside build artifacts; protection of running producers/consumers is proposed.
  A disposable declaration is still needed for arbitrary customer roots.
- No retention duration for durable Session history has been selected; it is
  excluded from automatic deletion in this design. Jack Heart asked about
  rotation; lossless cold archives are proposed separately from diagnostic expiry.
- Diagnostics defaults (seven days hot, 30 days retained, 4 GiB total) are proposed,
  not accepted. Native provider resume must survive any history archive scheme.

- Implementation choice, 2026-10-09: the first artifact contract is a valid
  `CACHEDIR.TAG` on a wholly ignored directory; unknown ignored data retains the
  checkout. No user-selected artifact roots or history-expiration policy are
  implied. Eight removals / 30 seconds admitting removals are conservative local
  defaults, not evidence of whole-pass timing or accepted machine-wide budgets.
- Source inspection found no `lf self uninstall` command. Schedule retirement
  needs the actual installation/disable boundary before the keystone can ship.
