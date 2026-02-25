# Open questions

- Should Stage 01 be considered complete with the documented headless blocker, or do we require an iOS UI automation target that exercises `connection setup → connect → wave list → wave detail → live output` in CI?
- Should we harden local/CI macOS UI testing by setting a dedicated `DerivedData` path and splitting UI tests from unit tests to reduce intermittent linker/code-signing failures?

## Rebase onto main blocked (2026-02-25)

Automated rebase aborted — conflicts are structural and need human judgment.

**Root cause:** PR #410 merged a large subset of this branch's work into main. Main then evolved those files.

**Second attempt (2026-02-25):** Confirmed the same failure mode. Commit-by-commit rebase of 25 commits hits cascading conflicts:
- Commit 1/25 (`design: mobile wave`): add/add conflicts on all 4 `wave/mobile/` docs. Resolved by accepting main's versions.
- Commit 2/25 (`kickoff`): applied cleanly.
- Commit 3/25 (`Clarify mobile phase scope`): conflicts again on `wave/mobile/` (3 files) plus `swift/README.md`. Every subsequent commit touching wave docs will re-conflict because each expects the prior branch state, not main's state.

This pattern repeats for most of the 25 commits. The wave docs were incrementally evolved across ~15 commits, producing ~15 rounds of cascading conflicts on the same files.

**Three conflict categories:**

1. **macOS file locations (architectural):** This branch has ~40 files in `Concerto/Platform/macOS/`. Main (after PR #410) moved them back to `Concerto/` root. Main has only 2 files in `Platform/macOS/`: `LocalShellCommandRunner.swift` and `RepoState+macOS.swift`.

2. **wave/mobile/ docs (cascading add/add):** Main has post-ship versions. The branch has 15+ intermediate versions. Net diff is small (4+21+7 lines across 3 files), but every intermediate commit conflicts.

3. **LoopflowCore state files:** `ConnectionStore.swift`, `OutputBuffer.swift`, `RepoState.swift` diverged. Net diff is ~100 lines of meaningful changes.

**Net divergence** (origin/main vs HEAD): 73 files, 441 insertions, 597 deletions. Most of the deletion delta is from the macOS file renames (2-line `#if os(macOS)` guard removal per file × 40 files).

**What needs deciding:**
1. Should macOS-only files live in `Concerto/Platform/macOS/` (this branch) or back in `Concerto/` root (main)? This is an architectural call that affects `project.yml` and the boundary check script.
2. Squash-rebase (one conflict resolution, loses per-commit history) vs. fresh branch from main with cherry-picked delta?
3. Or merge main into the branch instead of rebasing?

**Recommended approach:** Create a fresh branch from origin/main and manually apply the net delta (73 files). The 25-commit history is not worth preserving — it's iterative design/implementation that PR #410 already landed. What remains is the post-#410 refinements.
