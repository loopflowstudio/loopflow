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
- Implementation choice, 2026-10-09: admission starts after setup. Git observation
  subprocesses and independent registry SQL readers have two-second deadlines;
  candidate registry filesystem I/O, lease discovery and aggregate locked
  observation remain unbounded. Hint/receipt requests use isolated children
  with two-second deadlines plus bounded reaping; descriptor enumeration and spawn
  precede that clock. Per-registration setup now has its own 32-entry window,
  capped at five seconds admitting work and durably resumed before each read.
  Background setup now uses registration-local reads; manual listing/normalization
  is isolated. Remaining admission preparation is outside that bound. An admitted destructive
  removal is never canceled by these budgets.
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
- Implementation choice, 2026-10-09: read-only registration/hint/receipt requests
  and atomic scheduling writes use distinct subprocess operations, with neither
  checkout admission nor source-removal capability. Two-second request timeouts
  do not bound parent-side admission preparation. Lost receipt-write acknowledgments end that receipt's
  writer; later passes resume published progress under a new receipt. This does not
  select a final-history mechanism, cancel admitted removal, or bound inline lock
  discovery. Interrupted writes may leave administrative temporaries; retained-root
  artifact reclamation remains a follow-up.

- Implementation choice, 2026-10-09: background and locked observations no longer
  require the aggregate Git worktree listing; registration-local reads validate
  reciprocal administrative paths. Manual listings and settled-path discovery run
  in read-only workers, including path normalization. Remaining inline admission
  preparation and final evidence are named in the design. A stalled-gitdir fixture
  proves setup isolation and recovery, not Git removal through a stalled sibling.
  Item 2's final-history mechanism remains unselected.
