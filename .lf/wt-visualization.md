# Worktree Visualization Design

Status: implemented

## Worktree Storage

Worktrees use worktrunk-compatible sibling directory pattern:
- Pattern: `../{repo}.{branch}` with `/` replaced by `-`
- Example: `/Users/jack/src/loopflow.feature-auth`

## What We've Implemented

`lf wt list` now shows a table with:
- **Branch**: Name with tree connectors (├─, └─) showing PR dependency chains
- **St**: Status symbols (+!?) for staged/modified/untracked
- **main**: Ahead/behind main (↑N↓N or =)
- **PR**: PR number (#123) or "pushed"/"local"
- **Commit**: Short SHA
- **Age**: Relative time (e.g., "2 hours ago")
- **Message** (with `--full`): First line of commit message

Options:
- `--full` / `-f`: Include commit message column
- `--json`: Machine-readable output for scripting

## Feature Gap vs. worktrunk

worktrunk has several features we don't yet support. Here's what makes sense for loopflow:

### High Priority (Natural fit for loopflow)

1. **CI Status Integration**
   - worktrunk shows colored dots (green/red/blue/yellow) for CI status
   - For loopflow: `gh pr checks` already gives us this data
   - Display: `✓` (passed), `✗` (failed), `●` (running), `-` (none)
   - Fits loopflow: CI status matters for PR review/land decisions

2. **Clickable PR/CI Links**
   - Terminal hyperlinks (OSC 8) are widely supported now
   - PR column could link to the PR URL
   - CI indicator could link to the checks page
   - Fits loopflow: Faster navigation from terminal to GitHub

3. **Dimmed "Safe to Delete" Rows**
   - worktrunk dims worktrees that are merged and clean
   - We could dim worktrees where branch is deleted on origin (merged)
   - Fits loopflow: Visual cue for `lf wt clean` candidates

### Medium Priority (Useful but not core)

4. **Remote Sync Status**
   - Separate column for ahead/behind remote (vs. main)
   - Shows if local branch has unpushed commits
   - Mildly useful: We already show "pushed" vs "local"

5. **Line Diff Stats**
   - worktrunk shows +N -M lines changed
   - Could be helpful for gauging PR size
   - Implementation: `git diff --stat main...HEAD`

6. **Progressive Rendering**
   - worktrunk shows branch names immediately, fills in status as git commands complete
   - Only matters for very large repos
   - Probably over-engineering for loopflow's use case

### Low Priority (Out of scope)

7. **Worktree Creation/Deletion from List**
   - worktrunk has `wt new`, `wt delete`
   - loopflow already has `lf wt create`, `lf wt clean`
   - No gap to fill

8. **Branch-Only Listing**
   - worktrunk can show branches without worktrees
   - loopflow is specifically about worktrees for parallel agent work
   - Out of scope: Use `git branch` for that

9. **Rebase/Merge State Indicators**
   - worktrunk shows ⤴ (rebase) and ⤵ (merge) in progress
   - Rare edge case; user would know if they're mid-rebase
   - Nice to have but low priority

## Recommended Next Steps

1. **Add CI status** - biggest bang for buck
2. **Add terminal hyperlinks** - makes PR column actionable
3. **Dim merged branches** - helps identify cleanup candidates

## Implementation Notes

### CI Status

```python
# Get CI status from gh
result = subprocess.run(
    ["gh", "pr", "checks", "--json", "state", "-q", ".[].state"],
    cwd=path,
    capture_output=True,
    text=True,
)
# Aggregate: any FAILURE = ✗, all SUCCESS = ✓, any PENDING = ●, else -
```

### Terminal Hyperlinks

```python
def link(url: str, text: str) -> str:
    """Format as clickable terminal hyperlink (OSC 8)."""
    return f"\033]8;;{url}\033\\{text}\033]8;;\033\\"
```

### Detecting Merged Branches

A branch is "safe to delete" if:
1. Not dirty
2. Branch deleted on origin (not in remote_branches)
3. OR: `git merge-base --is-ancestor HEAD origin/main` succeeds

## Spirit of Loopflow

worktrunk is a general worktree manager. loopflow is specifically about:
- **Parallel agent workflows**: Multiple Claude/Codex instances on different tasks
- **PR-centric**: Everything is about getting changes into PRs
- **Comparison**: Comparing different implementations

Features should support these goals. Generic "git power user" features that don't help agent orchestration are out of scope.
