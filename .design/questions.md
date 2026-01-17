# Open Questions

## Resolved (2026-01-16)

1. **Target audience priority**: Power users for now. But progressive disclosure still matters—don't overwhelm with everything at once. The distinction: call things what they are (worktrees, not "workspaces"), but reveal complexity gradually.

2. **Results in terminal vs in-app**: Explore embedding an existing terminal (Ghostty, Warp) rather than building a competing terminal. Don't reimplement—integrate.

3. **Beta flag discoverability**: No discovery path needed. Beta features are in development; users won't see them until they're ready.

4. **Screenshot capture permissions**: Developer-only feature. Permissions will persist after initial grant—acceptable friction for developers.

5. **Worktree terminology**: Keep "worktree". It's pre-existing git terminology that Claude Code users already understand. Don't invent new language.

6. **Configuration vs opinionation**: Progressive disclosure, not dumbing down. Keep advanced options but reveal them gradually. Don't hide context toggles—but maybe collapse them initially. Use real terminology (worktrees, diff, tokens) but don't require understanding everything upfront.

9. **Sidebar identity**: Keep git-centric "branches" terminology. Power users think in git terms.

## Open

8. **In-app terminal embedding**: Can Maestro embed Ghostty or Warp rather than launching external? This would eliminate context-switching without competing with terminal products. See `.design/terminal-embedding.md` for research.

## Not Pursuing

7. **Task inference**: Decided against. Not worth the complexity—users can select tasks explicitly.
