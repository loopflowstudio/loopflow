# Decisions and assumptions — 2026-11-04

Jack Heart's accepted retention rules live in `cleanup-policy-decisions.md` and
`loo-722.md`. The coding-only laptop instruction supersedes older general
cache/Trash/history pruning. No new retention decision is needed.

- Unknown temp/demo roots remain observation-only. Pytest and known synthetic
  fixtures are disposable; other demos must mark their runtime explicitly.
  SQLite, age or size alone cannot make durable history disposable.
- LOO-725 retains provider history in unmarked demo Homes. Separating evidence
  from runtime remains unresolved; product demos do not auto-enroll in this job.
- Private versioned installation avoids dependence on any checkout; Chrome is
  a separate machine opt-in. Package installation enables neither.
- Unknown historical outcomes use seven days. Pytest/gate record observed
  outcomes. Native locks and recent/in-use records can leave caps unmet.
- Held `.venv`/`node_modules` remain regardless of age. Only eligible build
  output/disposable runtime permits owner teardown, with a 24h minimum.
- Root metadata can prove retention without a full tree walk. Unknown allocation
  stays null; concurrently disappearing candidates earn no reclaimed-byte credit.
- Discovery skips ignored data trees and follows registered worktrees. Hidden
  unregistered repositories remain a coverage limit. Background scans need 120s,
  within the 30-minute run and 100,000-directory bounds. Partial receipts fail.
- Batched `ls-files --others --ignored --exclude-standard --directory` plus
  `check-ignore --stdin -z` excludes falsely listed empty parents without tracked
  content filters. Per-directory queries were too slow; registry reads are reused.
- Fair usage sampling rotates by last attempt; deltas use the last known value
  and timestamp. Overlapping roots/APFS allocation do not prove physical causality.
- Near-term calendar acceptance temporarily adds an entry to the same hourly
  :17 job, then restores the installer-authored plist. This proves calendar
  invocation, not sustained hourly containment; it does not use kickstart.
- Manual gate recovery keeps its conservative seven-day rule while respecting
  saved markers and recent artifact content, so it cannot bypass the new evidence
  protections. This does not enroll other machines in scheduled cleanup.
