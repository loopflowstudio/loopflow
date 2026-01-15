# Open Questions

## 2026-01-15

No outstanding questions. Implementation is complete.

## Summary

The `worktreediffs` branch adds diff viewing and GitHub integration to Maestro:

### WorktreeService.swift
- `getDiff(for:in:base:)` - Runs `git diff base...branch` and returns output
- `getGitHubCompareURL(branch:in:base:)` - Parses git remote URL to build GitHub compare link

### WorktreeSidebar.swift
- `DiffSheet` - Modal view showing syntax-highlighted diff with loading state
- `DiffContentView` - Renders diff lines with color coding (green/red/cyan/blue)
- Hover actions on worktree rows now include diff viewer button and PR quick actions
- GitHub integration button opens compare URL in browser
