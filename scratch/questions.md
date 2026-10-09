# Open assumptions — 2026-10-09

- Mini comparison is incomplete; the access failures and requested address are
  recorded in [cleanup-evidence.md](cleanup-evidence.md).
- Proposed defaults (existing repository tick plus hourly full scan, eight checkouts per pass) are
  draft, not accepted decisions. Disk measurements will inform cache budgets.
- How broad should automatic ownership be for legacy taskless checkouts? Draft:
  report unless lf ownership and exact-head disposition are proven; never infer
  it from the directory name or branch author alone.
- The customer-facing contract for disposable build roots needs its follow-up
  design. Git-ignored files may contain credentials, datasets or local notes.
- No retention duration for durable Session history has been selected; it is
  excluded from automatic deletion in this design.
