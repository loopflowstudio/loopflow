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
  Installation settlement now repairs existing declarations before pruning old
  binaries; full installed-upgrade experience remains with demo.

- Implementation choice, 2026-10-09: ordinary-command fallback runs the existing
  finite cron command at most once per minute on work-producing entry points,
  never on prune previews. Unsupported hosts have no idle-time guarantee until
  they provide an OS scheduler; the fallback reports that limitation. Experiments
  neither install services nor activate fallback work automatically.
- Implementation choice, 2026-10-09: source, registration, registry, receipt and
  lock-file preparation run in isolated workers. Two-second request deadlines
  exclude descriptor enumeration/process launch and can add one second reaping;
  external execution retains its existing five-second budget. Workers transfer
  unlocked descriptors and exit before parent-owned nonblocking admission. No
  timed-out worker may acquire a checkout lock or continue into removal. Final
  history remains at its mechanism-review boundary; admitted removal is uncanceled.
- Implementation choice, 2026-10-09: candidate continuation is carried by at most
  32 pending administrative paths in the existing cron receipt, consumed before
  publication and drained before another setup window. Resuming that continuation
  skips repository-wide discovery. This replaces both fairness fields, not ownership
  or deletion evidence. Last-attempt priority applies within
  the remaining window. A fixed lexical sweep endpoint excludes later tail arrivals;
  arbitrary adversarial arrivals below that endpoint remain unproved.
- Implementation choice, 2026-10-09: one Session-history migration materializes raw
  references, backfilled 256 rows per tick to a fixed high-water mark. Source triggers
  maintain later appends/edits/deletes atomically. Complete coverage is required and
  destinations are resolved afresh under removal admission; partial/negative filesystem
  results are never cached. The final full reference read and native-symlink traversal
  can still exceed their deadlines on very large stores. This remains unfinished
  progress behavior, not a reason to relax evidence protection.
- Native historical-owner inspection found a provider home can contain a transcript
  symlink into a disposable checkout. Cleanup now checks the supported native layouts
  and the composed test invokes the real transcript-discovery reader after retention.
  Provider launch/resume and arbitrary provider archive layouts are not proved.
- Implementation choice, 2026-10-09: previews report `validate_checkout` for settled
  source requiring fresh history validation. Only locked application reads complete
  evidence and traverses native layouts. This conservative preview resolves the
  no-mandatory-foreground-traversal conflict without granting deletion authority.
- Architectural stop, 2026-10-09: the cost probe reaches the final-read deadline with
  65,536 already-projected rows. The fixed final scan cannot progress on that unchanged
  input; completed backfill is insufficient. Freshness forbids using accumulated
  negative filesystem results as authority. Final observation needs design review
  of workload-sized bounded observation plus isolated I/O, or a history-owner fresh
  view. Neither is selected here; no preservation constraint has been relaxed.
- Implementation choice, 2026-10-09: candidate-local positive settlement queries
  replace the registry-wide normalizing inventory. Historical aliases can miss
  minute selection but remain in hourly registration scans. Lookup failures yield
  one candidate and retry on the next sweep, not an incomplete complete set.
  This is scheduling only; fresh source/history checks retain disposal authority.
  Interrupted hint writes can leave small administrative temporaries; reclamation
  in retained roots remains an artifact follow-up. Preparation isolation does not
  solve final evidence progress or promise bounded destructive Git I/O.
- Review finding, 2026-10-09: isolating source observation bounds its I/O but
  still resolves the registry's complete Task checkout set and relevant Process
  cwd aliases. A stalled/unknown alias can retain unrelated candidates. The
  landing-path discovery fix does not prove this broader source-veto locality.
  Skipping unknown aliases is unsafe; a disjointness/ownership contract remains
  unselected. Dependent progress work stops with the existing mechanism review.
