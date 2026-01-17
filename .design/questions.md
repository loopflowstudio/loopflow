# Open Questions

## Resolved (2026-01-16)

1. **Target audience priority**: Power users for now. Maestro is a dashboard for people who already understand loopflow/Claude Code. Non-technical accessibility is not a priority.

2. **Results in terminal vs in-app**: Explore embedding an existing terminal (Ghostty, Warp) rather than building a competing terminal. Don't reimplement—integrate.

3. **Beta flag discoverability**: No discovery path needed. Beta features are in development; users won't see them until they're ready.

4. **Screenshot capture permissions**: Developer-only feature. Permissions will persist after initial grant—acceptable friction for developers.

5. **Worktree terminology**: Keep "worktree". It's pre-existing git terminology that Claude Code users already understand. Don't invent new language.

6. **Configuration vs opinionation**: Option (b)—accept Maestro is a power-user tool and optimize for that. Keep explicit context control visible.

9. **Sidebar identity**: Keep git-centric "branches" terminology. Power users think in git terms.

## Open

8. **In-app terminal embedding**: Can Maestro embed Ghostty or Warp rather than launching external? This would eliminate context-switching without competing with terminal products. See `.design/terminal-embedding.md` for research.

## Not Pursuing

7. **Task inference**: Decided against. Not worth the complexity—users can select tasks explicitly.
