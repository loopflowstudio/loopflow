# UX Agent

Improves Maestro first-run UX and accessibility based on comprehensive user research.

## Summary

- Conducted UX research simulating three user profiles (New Developer, Power User, Designer/PM)
- Implemented 8 targeted fixes addressing first-run experience and accessibility
- Added terminal embedding research for future in-app output streaming
- Updated UX prompts with build instructions

## UX Fixes Applied

1. **Default task to "implement"** - Most common case; users don't have to choose
2. **Improved placeholder text** - More prominent, with concrete examples
3. **Renamed "BRANCHES" to "Workspaces"** - Friendlier, less git-jargon
4. **Added icons to stage badges** - Accessibility fix for color-blind users
5. **Better task descriptions** - 2-line limit with proper wrapping
6. **Better collapse indicator** - "More options" when collapsed, with tooltip
7. **Removed redundant repo name** - Was in both title and toolbar
8. **Concrete welcome tagline** - "Tell it what to build. It writes the code."

## Remaining Work

High-complexity items deferred for future:
- In-app terminal embedding (see `.design/terminal-embedding.md`)
- Onboarding flow for first-time users
- Cmd+K command palette
- Slash commands for task selection
- @ mentions for context

## Artifacts

- `.design/ux-research.md` - User profile simulations and pain points
- `.design/ux-gaps.md` - Gap analysis vs Figma/Cursor/Notion
- `.design/ux-fixes.md` - Detailed changelog of fixes
- `.design/terminal-embedding.md` - Research on SwiftTerm/Ghostty
- `.design/questions.md` - Product decisions
