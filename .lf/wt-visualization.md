# Worktree Visualization Design

Status: implemented

## Worktree Storage

Worktrees use worktrunk-compatible sibling directory pattern:
- Pattern: `../{repo}.{branch}` with `/` replaced by `-`
- Example: `/Users/jack/src/loopflow.feature-auth`

## What We've Implemented

`lf wt list` now shows a table with:
- **Branch**: Name with tree connectors (├─, └─) showing PR dependency chains
- **St**: Status symbols (+!?⤴⤵) for staged/modified/untracked/rebase/merge
- **main**: Ahead/behind main (↑N↓N or =)
- **remote**: Ahead/behind remote tracking branch (↑N↓N, =, or - if local)
- **CI**: CI status (✓ passed, ✗ failed, ● running, - none) with clickable link
- **PR**: PR number (#123, clickable link) or "pushed"/"local"
- **Diff**: Line diff stats vs main (+N -M)
- **Commit**: Short SHA
- **Age**: Relative time (e.g., "2 hours ago")
- **Message** (with `--full`): First line of commit message

Features:
- Dimmed rows for worktrees safe to delete (merged or branch gone from origin)
- Terminal hyperlinks (OSC 8) on PR numbers and CI indicators
- JSON output includes all fields for scripting

Options:
- `--full` / `-f`: Include commit message column
- `--json`: Machine-readable output for scripting

## Feature Gap vs. worktrunk

worktrunk has several features we don't yet support. Here's what makes sense for loopflow:

### High Priority (Natural fit for loopflow) - DONE

1. **CI Status Integration** ✓
   - Display: `✓` (passed), `✗` (failed), `●` (running), `-` (none)
   - Fetched from `gh pr checks`

2. **Clickable PR/CI Links** ✓
   - Terminal hyperlinks (OSC 8) on PR numbers and CI indicators
   - PR links to PR URL, CI links to checks page

3. **Dimmed "Safe to Delete" Rows** ✓
   - Worktrees where branch is gone from origin or merged are dimmed
   - Visual cue for `lf wt clean` candidates

### Medium Priority (Useful but not core) - DONE

4. **Remote Sync Status** ✓
   - Separate "remote" column shows ahead/behind remote tracking branch
   - Shows ↑N/↓N or = if in sync, - if local-only

5. **Line Diff Stats** ✓
   - "Diff" column shows +N -M lines changed vs main
   - Helpful for gauging PR size

### Skipped (Out of scope)

6. **Progressive Rendering**
   - worktrunk shows branch names immediately, fills in status as git commands complete
   - Only matters for very large repos
   - Over-engineering for loopflow's use case

### Low Priority - DONE

7. **Worktree Creation/Deletion from List**
   - worktrunk has `wt new`, `wt delete`
   - loopflow already has `lf wt create`, `lf wt clean`
   - No gap to fill

8. **Branch-Only Listing**
   - worktrunk can show branches without worktrees
   - loopflow is specifically about worktrees for parallel agent work
   - Out of scope: Use `git branch` for that

9. **Rebase/Merge State Indicators** ✓
   - St column shows ⤴ (rebase) and ⤵ (merge) in progress
   - Detected via .git/rebase-merge, .git/REBASE_HEAD, .git/MERGE_HEAD

## Spirit of Loopflow

worktrunk is a general worktree manager. loopflow is specifically about:
- **Parallel agent workflows**: Multiple Claude/Codex instances on different tasks
- **PR-centric**: Everything is about getting changes into PRs
- **Comparison**: Comparing different implementations

Features should support these goals. Generic "git power user" features that don't help agent orchestration are out of scope.
