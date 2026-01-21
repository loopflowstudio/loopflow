# Land Button as Primary Action

A prominent Land button replaces PR as the primary shipping action in Maestro's worktree detail panel. Draft PRs are auto-created by lfd so a PR always exists when there's work to land.

## What to build

1. Reorganize quick actions bar: Land becomes primary, PR becomes "View PR"
2. Move draft PR creation from Maestro to lfd (faster, works without Maestro open)
3. Disable Land when diff is empty (not just when no commits)

## Current State

```swift
// WorktreeDetailPanel.swift quickActionsBar
[PR] [Cursor] [Warp]     ...    [Land (only if prState == .open)] [Abandon]
```

- PR button always visible, creates/opens PR
- Land button only visible when an open PR exists

## New Design

```swift
// quickActionsBar
[Land] [Cursor] [Warp]     ...    [View PR (only if prNumber != nil)] [Abandon]
```

**Land button behavior:**
- Draft PR → mark ready + `lfops land` (enables auto-merge)
- Open PR → `lfops land` (enables auto-merge)
- Enabled when there's a diff to land (`hasDiff == true`)
- Note: lfd auto-creates draft PRs, so a PR should always exist when there's work

**View PR button:**
- Only visible when `prNumber != nil`
- Disabled when `hasDiff == false` (nothing to review)
- Opens PR URL in browser
- Shows PR number in label: "PR #42"

## Data Structures

Add `hasDiff` to Worktree model (computed from existing `diffVsMain` JSON):

```swift
// Worktree.swift
struct Worktree {
    // ... existing properties ...
    let hasDiff: Bool  // True if diff_vs_main has changes
}

extension Worktree {
    init(from json: WorktreeJSON, ...) {
        // ... existing init ...
        let diffStats = json.workingTree?.diffVsMain
        self.hasDiff = (diffStats?.added ?? 0) + (diffStats?.deleted ?? 0) > 0
    }
}
```

Uses existing properties:
- `prNumber: Int?` - determines View PR visibility
- `prState: PRState?` - determines Land button behavior
- `hasDiff: Bool` - determines if Land is enabled

## Key Functions

```swift
// WorktreeDetailPanel.swift

private var quickActionsBar: some View {
    HStack(spacing: 12) {
        // Land button - primary action, always first
        landButton

        Button { openInIDE() } label: { ... }
        Button { openInTerminal() } label: { ... }

        Spacer()

        // View PR - only when PR exists
        if worktree.prNumber != nil {
            viewPRButton
        }

        Button { showingAbandonConfirmation = true } label: { ... }
    }
}

private var landButton: some View {
    Button {
        landBranch()
    } label: {
        HStack(spacing: 8) {
            Image(systemName: "airplane.arrival")
            Text("Land")
                .fontWeight(.medium)
        }
        .frame(minWidth: 70)
    }
    .buttonStyle(DarkButtonStyle())
    .disabled(!worktree.hasDiff)
    .help(landButtonHelp)
}

private var landButtonHelp: String {
    if !worktree.hasDiff {
        return "No changes to land"
    }
    if worktree.prState == .draft {
        return "Mark PR ready and enable auto-merge"
    }
    return "Enable auto-merge for PR"
}

private var viewPRButton: some View {
    Button {
        if let url = worktree.prURL {
            terminalLauncher.openURL(url)
        }
    } label: {
        HStack(spacing: 8) {
            Image(systemName: "arrow.up.right.square")
            Text("PR #\(worktree.prNumber ?? 0)")
                .fontWeight(.medium)
        }
        .frame(minWidth: 70)
    }
    .buttonStyle(DarkButtonStyle())
    .disabled(!worktree.hasDiff)
    .help(worktree.hasDiff ? "View PR on GitHub" : "No changes to review")
}

private func landBranch() {
    Task {
        do {
            try await appState.landBranch(for: worktree)
        } catch {
            actionError = "Failed to land: \(error.localizedDescription)"
            showingActionError = true
        }
    }
}
```

```swift
// AppState.swift

func landBranch(for worktree: Worktree) async throws {
    let worktreeURL = URL(fileURLWithPath: worktree.path)
    try await worktreeService.landBranch(in: worktreeURL)
    listWorktrees()
}
```

```swift
// WorktreeService.swift

func landBranch(in worktreePath: URL) async throws {
    // lfd auto-creates draft PRs, so we just need to land
    // lfops land handles: marking draft ready, enabling auto-merge
    _ = try await runLfops(["land"], in: worktreePath)
}
```

```python
# src/loopflow/lfd/scheduler.py

async def auto_create_draft_prs(self, repo: Path) -> None:
    """Create draft PRs for branches with pushed changes but no PR."""
    worktrees = list_worktrees(repo)
    for wt in worktrees:
        if wt.branch in ("main", "master"):
            continue
        if wt.pr_number is not None:
            continue
        if not wt.has_diff:
            continue
        if not branch_is_pushed(wt.branch, repo):
            continue
        # Create draft PR
        subprocess.run(
            ["gh", "pr", "create", "--draft", "--fill"],
            cwd=wt.path,
            capture_output=True,
        )
```

Remove from Maestro:
- Delete `createDraftPRsIfNeeded` from `AppState.swift`
- Delete call to it from `syncAndEnrich`

## UI Changes

**Quick actions bar layout change:**

Before:
```
[PR]  [Cursor]  [Warp]        [Land*]  [Abandon]
                              * only when prState == .open
```

After:
```
[Land]  [Cursor]  [Warp]      [PR #42*]  [Abandon]
                              * only when prNumber != nil
```

**Button states:**
- Land: disabled when `hasDiff == false`
- View PR: visible when `prNumber != nil`, disabled when `hasDiff == false`

**Button styling:** Both use existing `DarkButtonStyle`. Land button in first position gives it visual prominence.

## Constraints

- lfd must auto-create draft PRs for branches with pushed changes
- `lfops land` must handle draft and open PRs (mark ready if draft, enable auto-merge)
- View PR button only visible when PR exists, disabled when no diff
- Land button disabled when no diff
- `wt list --full` must be used to get `diff_vs_main` data

## Done when

**Maestro:**
1. Build succeeds: `cd Maestro && xcodebuild -scheme Maestro build`
2. Land button appears first in quick actions bar
3. Land button is disabled when `hasDiff == false`
4. View PR button only visible when `prNumber != nil`
5. View PR button is disabled when `hasDiff == false`
6. Clicking Land on draft PR marks ready and enables auto-merge
7. Clicking Land on open PR enables auto-merge
8. `createDraftPRsIfNeeded` removed from AppState

**lfd:**
9. Draft PRs auto-created for branches with pushed changes and no PR
10. Auto-draft runs on scheduler tick (same as other periodic tasks)
