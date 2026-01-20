# Worktree Dashboard

Workflow-focused dashboard replacing the worktree detail panel: quick actions bar, commits, GitHub-style file diff summary with per-file quick actions, launcher collapsed by default.

## Review

**Verdict:** Ready to ship

## Design notes

**Layout (top to bottom):**
1. Header — branch name, PR badge, commit count
2. Quick actions bar — PR, IDE, Terminal, Land (when open PR), Abandon
3. Commits section — expanded by default
4. Changed Files section — GitHub-style diff summary sorted by changes
5. Launcher section — collapsed by default

**Key decisions:**
- History section removed (user feedback: not needed for workflow)
- Main branch filtered from sidebar (worktree view focuses on feature branches)
- Section states persist via `@AppStorage`
- Land and Abandon buttons visible in both sidebar hover actions and detail panel
- PR button runs `lfops pr` which handles both creation and opening existing PRs
- Status indicators (running/dirty/clean) replaced with commit count badge in sidebar

**Draft PR strategy:**
- Auto-create draft PRs on refresh for pushed branches without PRs (Maestro fallback)
- PR button upgrades draft to ready-for-review, then opens browser
- Faster UX since draft already exists when user clicks PR button

**lf post-push draft creation:**
- `ensure_draft_pr()` in `lf/git.py` creates draft PR if none exists
- Called after push in `add_commit_push()`, `autocommit()`, and `lfops commit -p`
- Maestro's refresh-based creation remains as fallback for branches pushed outside lf

**Git log parsing:**
- Message field moved to last position in format string so pipes in commit messages don't break parsing
