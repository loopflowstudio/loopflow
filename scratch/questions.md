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
- Source inspection found no `lf self uninstall` command; the existing retirement
  boundary is `lf wave cron sync --repo --disable`. Implementation assumption:
  automatic activation and ordinary-command fallback must respect explicit disable;
  that choice is now persisted; executable-definition repair is tested.
  Promotion-time repair still needs implementation; installed-upgrade experience
  remains with demo.

- Implementation choice, 2026-10-09: ordinary-command fallback runs the existing
  finite cron command at most once per minute on work-producing entry points,
  never on prune previews. Unsupported hosts have no idle-time guarantee until
  they provide an OS scheduler; the fallback reports that limitation. Experiments
  neither install services nor activate fallback work automatically.
- The 30-second budget currently bounds admission of candidate observations and
  removals, not initial snapshots or a Git/SQL call already in progress. Hard read
  deadlines, hourly scans, fair ordering and size measurement remain internal
  work in this PR. These are implementation gaps, not a proposed relaxation of
  bounded maintenance.
