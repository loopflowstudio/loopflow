# Open questions and assumptions (Task 647863)

- **Window length.** The Task says "once per budget window". Chosen:
  max(10 minutes, `delta_fresh_secs()`), so a healthy budget retries a stuck
  creation every ten minutes and a paced or reserved budget waits for its
  pacing interval or reset. Linear's hourly allowance would also fit "window";
  ten minutes keeps a Task created during a short outage from waiting an hour.
- **Auto-retry applies to Projects too.** Project creation uses the same
  receipt and saved id; the policy is uniform. Attachment retries stay explicit.
- **Pre-write check removed from every mutation**, not only creation. Writes
  were already unconditional; freshness runs at command boundaries and before
  state delivery. If a field or comment write needs a fresh delta later, call
  `check_before_write` there explicitly.
- Wave memory `.lf/wave/infrastructure/MEMORY.md` was 89 tokens over its
  16,000-token target. Compress merged duplicated "remains unproved" lists and a
  pass ledger already kept at `6935bfbadbc2…` into their references; realign
  (2026-11-05) replaced the stale "live request counts remain unproved" paragraph
  under On-demand Linear acquisition with this Task's measured counts and lessons.
  No decision or attribution changed.
