# Open local-build questions

- Jack Heart accepted practical completed-worktree cleanup on October 13. The
  native Cargo lock-replacement race is a documented limitation, not a permission
  blocker. Implement positive completed ownership, old output, fresh process and
  execution checks, unknown/busy retention and cooperating leases. A detected
  Cargo process raises a candidate-selection concern and must retain output.
  Concurrent direct tools ignoring cleanup coordination are unsupported.
- An exploratory prepared startup run passed with nextest's `LEAK` warning;
  cause is unconfirmed. Preserve its raw log alongside clean runs rather than
  claiming that CLI preparation repaired descendant/pipeline lifetime.
- Broader Infrastructure overlap remains unknown after failed plan inspection;
  the design records the confirmed sibling scope.

The [research](local-build-research.md) owns cold-cache assumptions and probe
evidence. The [design](local-build-speed-space.md) owns the October 13 draft policy:
24h without modification, eligibility at the next supported invocation, the accepted practical concurrency policy, and history/store/install exclusions.

The installed `lf context` omits relocated `.lf/wave/infrastructure/MEMORY.md`.
Memory is measured separately with cl100k_base against its reported 16,000-token /
131,072-byte defaults; scratch uses the normal query. No limits were changed.

Implementation uses native modification times for the 24-hour grace and counts
unique `(device, inode)` allocations per generated root. Native metadata/ownership
uncertainty retains output. The direct-Cargo probe uses a compiler shim only to
keep actual Cargo observable; no replaceable Cargo lock grants deletion authority.
Infrastructure memory includes another contribution, preserved outside this Task's
checkpoints. Reconciliation updates only its local-build lessons; the installed
context query still omits the relocated memory, so it is measured separately.
