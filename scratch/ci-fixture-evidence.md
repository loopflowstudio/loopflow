# CI fixture evidence

2026-10-01, review during Jack Heart's authorized landing of PR #1390.

Both failing fixtures register a Task whose worktree is the launch checkout, then call a Session without explicit Task attribution an orphan. `store/sqlite/task_work.rs::session_membership` includes checkout membership independently of explicit binding. `finish_session_driver` deliberately preserves that work through `NOT EXISTS(session_tasks("s"))`.

On Linux, `tasks.worktree` and the launched Session cwd have matching spellings, so these conversations are retained correctly. On macOS, `TestRepo::path()` retains `/var/...` while process cwd canonicalizes to `/private/var/...`; the fixture's membership differs. The earlier PR-resume fixture had the same alias discrepancy in seeded planning and was corrected in pr_tests.rs.

Likely repair: use a genuinely separate unowned checkout for orphan/bind-after-retirement proof; keep same-checkout conversations open and prove their Task membership independently from immutable usage attribution. Canonicalize fixture Task paths so macOS tests exercise the same membership as Linux. Production retirement must continue to preserve checkout-associated Task work. This is evidence from SQL and fixture setup, not a second implementation in progress.
