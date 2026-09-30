# LOO-355 lifecycle decisions and acceptance boundary

2026-09-30 — Jack Heart requested composable lifecycle operations and issue-addressed
action coverage. The latest supervising steer assigns completion cleanup, deletion
and recovery reconciliation here, with configured live proofs after landing.

## Implementation decisions

- `task pr ISSUE ACTION` uses the existing PR parser. `task sync ISSUE` now uses
  main's sole integration command; the temporary Task `rebase` spelling is removed.
- Bare Task landing retains its open outcome and checkout for the saved Flow or
  next PR, and reports that explicitly. Completing landing composes merge,
  Task/Linear success and branch cleanup. A standalone landed PR deletes its
  checkout and branches. Primary checkouts remain protected and reported.
- Completion records its outcome before cleanup. Claimed workers keep their
  captured boundary until their provider stops and the exact worker settles.
  A dead worker can be settled on explicit completion retry; live or unknown
  execution defers cleanup. No self-wait, new lease table or migration is added.
- Empty successors are retired as abandoned PR history in the completion
  transaction, preserving the branch identity needed for cleanup retries.
- `task delete ISSUE` composes cancellation for unfinished placed work, completion
  cleanup for Done work, and provider trash. It refuses live/unknown workers and
  dirty work. Planning-only deletion allocates nothing. Retained history and
  provider trash confirmations remain authoritative on retry.
- Recovery means checkout recovery, saved execution continuation, or retrying a
  partial lifecycle operation. It never reopens a terminal outcome or interprets
  missing membership as cancellation. No new terminal-outcome reopening authority
  was requested; that remains an explicit future product decision.

These are implementation choices within Jack Heart's requested composition,
not a separate recorded acceptance of the finished behavior.

## Configured acceptance after landing

The earlier 0.12.26 parser observations are historical. Jack Heart's supervising
steer reports v0.12.27 installed and explicitly places the disposable configured
Task proof and Wave sweep preview/apply after this change lands, using installed
`lf`. The branch was synced through `lf sync` (merge). No source binary has been
used against the installed store and no promotion or provider cancellation was
performed in this implementation pass.

The caller owns publication, landing, installation and Flow navigation. After
installation: prove abandonment on a disposable Task across Linear/store/GitHub/Git,
preview all configured Waves, then apply eligible cancellations including LOO-309
and LOO-329. Missing evidence and exclusions remain visible; never substitute a
separate Linear writer or database patch for the composed operation.
