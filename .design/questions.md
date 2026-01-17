# Open Questions

## Resolved (2026-01-16)

1. **Target audience priority**: Power users for now. But progressive disclosure still matters—don't overwhelm with everything at once. Reveal complexity gradually while keeping the UI approachable.

2. **Results in terminal vs in-app**: Explore embedding an existing terminal (Ghostty, Warp) rather than building a competing terminal. Don't reimplement—integrate.

3. **Beta flag discoverability**: No discovery path needed. Beta features are in development; users won't see them until they're ready.

4. **Screenshot capture permissions**: Developer-only feature. Permissions will persist after initial grant—acceptable friction for developers.

5. **Worktree terminology**: Use "Workspaces" in the UI for approachability. The underlying git concept is worktrees, but "workspace" is more intuitive for users who aren't git experts. Code and CLI can still use "worktree" terminology.

6. **Configuration vs opinionation**: Progressive disclosure, not dumbing down. Keep advanced options but reveal them gradually. Don't hide context toggles—but maybe collapse them initially. Use real terminology (worktrees, diff, tokens) but don't require understanding everything upfront.

9. **Sidebar identity**: Changed from "BRANCHES" to "Workspaces" for friendlier tone. The all-caps semibold was visually aggressive; medium weight title case is more approachable.

## Open

8. **In-app terminal embedding**: Can Maestro embed Ghostty or Warp rather than launching external? This would eliminate context-switching without competing with terminal products. See `.design/terminal-embedding.md` for research.

## Not Pursuing

7. **Task inference**: Decided against. Not worth the complexity—users can select tasks explicitly.
