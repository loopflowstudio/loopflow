# Design Review: Waiting State and PR Collapse

This branch implements actionable waiting states and PR collapse functionality for waves blocked by PR limits.

## What was implemented

1. **Actionable waiting state UI** — When a wave hits its PR limit, users now see exactly why (e.g., "2/5 PRs open") with two immediate actions: "Review PRs" opens GitHub's PR list, "Collapse into One" combines all wave PRs into a single PR.

2. **PR collapse backend** — New `collapse_prs()` function in `wave.py` that:
   - Fetches open PRs for the wave from GitHub
   - Creates a new branch from main
   - Squash-merges each PR's changes
   - Opens a combined PR
   - Closes old PRs with link to new one
   - Deletes old remote branches
   - Auto-resumes the wave

3. **HTTP endpoint** — `POST /waves/{wave_id}/collapse` exposes the collapse operation via the daemon API.

4. **WaitingReason model** — New enum in Swift with `prLimitReached(open:limit:)` case. Includes both compact display (`2/5 PRs open`) and accessible description (`2 of 5 PRs open`).

5. **Nested timestamp parsing fix** — `_remove_doubled_prefix()` handles corrupted branch names where the wave name got duplicated (e.g., `jack.rust.jack.rust.20260130`).

## Key choices

**Squash merge over regular merge.** Each PR branch gets squash-merged into the collapsed branch. This produces a clean linear history instead of preserving individual commits. Trade-off: individual commit history is lost, but the collapsed PR is easier to review.

**Best-effort cleanup.** Old PR branches are deleted after collapse, but failures don't abort the operation. The main goal (creating a combined PR) succeeds even if branch cleanup fails.

**Blocking subprocess calls via asyncio.to_thread.** The collapse operation does many sequential git/gh calls. Rather than making everything async, we run the whole thing in a thread pool to avoid blocking the event loop.

**Swift Process for git remote.** The `WaitingStateCard` extracts owner/repo by shelling out to git. This duplicates logic that exists in Python, but keeps the Swift code self-contained without adding new HTTP endpoints.

## How it fits together

```
User sees waiting state → clicks "Collapse into One"
       ↓
WaitingStateCard calls WaveService.collapsePRs()
       ↓
HTTP POST /waves/{id}/collapse
       ↓
collapse_prs() in wave.py:
  - gh pr list → filter to wave's branches
  - git checkout -b collapsed-branch origin/main
  - for each PR: git merge --squash origin/{branch}
  - git commit + push
  - gh pr create (combined)
  - gh pr close (old PRs)
  - git push --delete (old branches)
  - update wave status → IDLE
       ↓
UI shows new PR URL, wave resumes
```

## Risks and bottlenecks

**Merge conflicts.** If PRs touch the same files, the squash merge will fail. The function aborts cleanly and reports which branch conflicted, but there's no automatic resolution.

**GitHub rate limits.** Collapse does multiple gh CLI calls (list, close, create). Heavy use could hit rate limits, though unlikely in practice.

**Long-running operation.** Multiple git/gh calls can take 10-30 seconds. The HTTP endpoint uses `longSession` with extended timeout, and the UI shows a spinner.

**No undo.** Once collapsed, old PRs are closed and branches deleted. Users would need to manually recreate from git reflog if they want to reverse.

## What's not included

**Partial collapse.** Can't select which PRs to collapse — it's all or nothing for the wave.

**Conflict resolution.** No interactive merge conflict handling. Users must manually resolve and retry.

**Non-GitHub remotes.** Only works with GitHub repositories (the gh CLI is required).

**Offline support.** Requires network connectivity for all GitHub operations.

## Files changed

| File | Change |
|------|--------|
| `src/loopflow/lfd/wave.py` | `CollapsePRsResult`, `collapse_prs()`, `_parse_github_remote()` |
| `src/loopflow/lfd/daemon/http_server.py` | `/waves/{id}/collapse` endpoint, waiting reason in wave dict |
| `swift/LoopflowCore/Models/Wave.swift` | `WaitingReason` enum, `waitingReason` property |
| `swift/LoopflowCore/Services/WaveService.swift` | `collapsePRs()`, waiting reason parsing, `CollapsePRsResult` |
| `swift/Concerto/Views/WaitingStateCard.swift` | New view for actionable waiting state |
| `swift/Concerto/Views/WaveDetailPanel.swift` | Uses `WaitingStateCard` for waiting waves |
| `tests/test_lfd.py` | Tests for `_parse_github_remote()`, `CollapsePRsResult` |
| `swift/ConcertoTests/WaveTests.swift` | Tests for `WaitingReason`, `CollapsePRsResult` |

Also includes naming fixes from prior work:
- `src/loopflow/lf/naming.py` — `_remove_doubled_prefix()` for corrupted branch names
- `tests/test_naming.py` — Tests for doubled prefix removal
- `src/loopflow/lf/ops/next.py` — Uses deduplication in worktree preservation
- `tests/test_next.py` — Tests for worktree preservation with doubled names
