# Land Button as Primary Action

A prominent Land button replaces PR as the primary shipping action in Maestro's worktree detail panel.

## What to build

Reorganize the quick actions bar: Land becomes the primary action button, PR becomes "View PR" (only visible when a PR exists).

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
- No PR → `lfops land --create-pr` (creates PR + enables auto-merge)
- Draft PR → mark ready + `lfops land` (enables auto-merge)
- Open PR → `lfops land` (enables auto-merge)
- Always enabled when there are commits (aheadMain > 0)

**View PR button:**
- Only visible when `prNumber != nil`
- Opens PR URL in browser
- Shows PR number in label: "PR #42"

## Data Structures

No new data structures. Uses existing `Worktree` properties:
- `prNumber: Int?` - determines View PR visibility
- `prState: PRState?` - determines Land button behavior
- `aheadMain: Int` - determines if Land is enabled

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
    .disabled(worktree.aheadMain == 0)
    .help(landButtonHelp)
}

private var landButtonHelp: String {
    if worktree.aheadMain == 0 {
        return "No commits to land"
    }
    if worktree.prNumber == nil {
        return "Create PR and enable auto-merge"
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
    .help("View PR on GitHub")
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
    // Uses --create-pr to handle all cases:
    // - No PR: creates PR + enables auto-merge
    // - Draft PR: lfops land handles marking ready
    // - Open PR: enables auto-merge
    _ = try await runLfops(["land", "--create-pr"], in: worktreePath)
}
```

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

**Land button states:**
- Disabled (gray): no commits (`aheadMain == 0`)
- Enabled: has commits to land

**Button styling:** Both use existing `DarkButtonStyle`. Land button in first position gives it visual prominence.

## Constraints

- `lfops land --create-pr` must handle all cases (no PR, draft PR, open PR)
- The View PR button must only appear when there's a PR to view
- Land button must be disabled when there's nothing to land (no commits)

## Done when

1. Build succeeds: `cd Maestro && xcodebuild -scheme Maestro build`
2. Land button appears first in quick actions bar
3. Land button is disabled when `aheadMain == 0`
4. View PR button only appears when PR exists
5. Clicking Land on a branch with no PR creates PR and enables auto-merge
6. Clicking Land on a branch with draft PR marks ready and enables auto-merge
7. Clicking Land on a branch with open PR enables auto-merge
