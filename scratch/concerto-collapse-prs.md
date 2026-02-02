# Collapse Outstanding PRs

Combine multiple outstanding PRs from a wave into a single PR, with full cleanup.

## Status

Implemented. See files changed below.

## Context

When a wave hits its PR limit (e.g., "2/5 PRs open"), the user currently has two options:
1. Review and merge PRs individually
2. Stop the wave

Sometimes neither is ideal—the PRs are small incremental changes that would review better as one cohesive PR.

## What was built

Add a "Collapse PRs" action to the WaitingStateCard that:

1. **Fetches open PRs** for this wave from GitHub API
2. **Creates a new branch** from main with all changes squashed
3. **Opens a single PR** with combined description
4. **Closes old PRs** with comment linking to the new one
5. **Deletes old remote branches**

### UI

WaitingStateCard gets a second button:

```
┌─────────────────────────────────────────┐
│ ⏸ Waiting                               │
│                                         │
│ 2/5 PRs open                            │
│                                         │
│ [Review PRs]  [Collapse into One]       │
└─────────────────────────────────────────┘
```

### Flow

```
User clicks "Collapse into One"
    │
    ▼
Fetch open PRs for wave (gh pr list --author @me --json)
    │
    ▼
Confirm dialog: "Collapse 2 PRs into one? This will close PR #123, #124"
    │
    ▼
Create new branch: wave-name-collapsed-YYYYMMDD
    │
    ▼
Cherry-pick or merge all PR branches
    │
    ▼
Squash into single commit
    │
    ▼
Push and create new PR (gh pr create)
    │
    ▼
Close old PRs with comment (gh pr close --comment)
    │
    ▼
Delete old remote branches (git push origin --delete)
    │
    ▼
Update wave metadata to track new branch
```

### Implementation

**Python daemon** (`lfd`):
- New endpoint: `POST /waves/{id}/collapse`
- Orchestrates the git/gh operations
- Returns new PR URL on success

**Swift UI**:
- Add "Collapse into One" button to WaitingStateCard
- Confirmation dialog showing which PRs will be collapsed
- Progress indicator during operation
- Success: open new PR in browser
- Error: show what failed, which PRs remain

### Git operations

```bash
# Fetch PR branches
gh pr list --author @me --repo owner/repo --json number,headRefName,title

# Create collapsed branch from main
git checkout -b wave-collapsed-20260202 origin/main

# Merge each PR branch (or cherry-pick)
git merge --squash origin/pr-branch-1
git merge --squash origin/pr-branch-2

# Commit with combined message
git commit -m "feat: combined changes from wave

Collapses:
- #123: first change
- #124: second change"

# Push and create PR
git push -u origin wave-collapsed-20260202
gh pr create --title "Combined: wave changes" --body "..."

# Close old PRs
gh pr close 123 --comment "Collapsed into #125"
gh pr close 124 --comment "Collapsed into #125"

# Delete old branches
git push origin --delete pr-branch-1 pr-branch-2
```

### Edge cases

- **Merge conflicts**: If PR branches conflict with each other, abort and tell user which PRs conflict
- **PR has reviews/comments**: Warn user that review context will be lost (link in close comment preserves reference)
- **Branch protection**: New PR still needs to pass CI, user still needs to get reviews
- **Partial failure**: If we close PRs but fail to delete branches, that's acceptable (branches are just cleanup)

## Done when

User can collapse 2+ outstanding PRs into one with a single click, and the old PRs/branches are cleaned up.

## Decisions

- **All PRs**: Always collapse all outstanding PRs (no picker for v1)
- **Auto-resume**: Wave resumes automatically after collapse (it was blocked on limit, now under)
- **Description**: Link to old PRs, generate new description via `generate_pr_message`

## Files changed

| File | Change |
|------|--------|
| `src/loopflow/lfd/wave.py` | `CollapsePRsResult` dataclass, `collapse_prs()` function, `_parse_github_remote()` helper |
| `src/loopflow/lfd/daemon/http_server.py` | `POST /waves/{wave_id}/collapse` endpoint |
| `swift/LoopflowCore/Services/WaveService.swift` | `collapsePRs()` method, `CollapsePRsResult` struct |
| `swift/Concerto/Views/WaitingStateCard.swift` | "Collapse into One" button, confirmation dialog, loading state |
| `swift/ConcertoTests/WaveTests.swift` | Tests for `CollapsePRsResult` |
