# LOO-1108 verify (2026-11-25)

PR #2049 merged as d8b2afebd9; worktree HEAD tree == origin/main tree (`git diff --stat HEAD origin/main` empty).
Built main `lf` (debug), ran with private LF_HOME=/tmp/lfv.yxp2/home, LF_* unset.

- `lf graph list`: code "Build and deliver. You review the change before it merges."; feature "Explore, build and deliver. ..."
- `lf graph list --json`: code ["You take part at ship-review.",["plan","build","ship-review","deliver"]]; feature ["You take part at explore and ship-review.",["explore","plan","build","ship-review","deliver"]]
- `lf verify`: "error: skill or graph not found: verify."
- `git grep` on origin/main: no `interactive: verify`, no builtin verify.md, no .lf/skills/verify.md; only hit is the `retired_skills_are_not_registered` test asserting absence.

CI on d8b2afebd9: merge_group success; push `CI` in progress, `macOS on main` queued at check time (unread result).
Follow-ups already recorded in PR: other repos (manabot, fantasia, etude, etude-sdk) need hand edits; Task 559762 (Desktop merge hint).
Outcome: Verified.
