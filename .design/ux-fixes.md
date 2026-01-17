# UX Fixes Applied

## Prompt Input: Default task to "implement"
**Problem**: Task selector required explicit choice, no default. New users didn't know which task to select.
**Change**: Auto-select "implement" task when prompts load. This is the most common case - users want to build something.
**Files**: `Maestro/Maestro/Views/PromptLauncher.swift`

## Prompt Input: Improve placeholder text
**Problem**: Placeholder "Describe what you want to build..." at `.tertiary` was too faded. First thing users see should be more prominent.
**Change**: Changed placeholder to "What should the AI build?" at `.secondary` opacity, with better example text showing concrete use cases.
**Files**: `Maestro/Maestro/Views/PromptLauncher.swift`

## Sidebar: Rename "BRANCHES" to "Workspaces"
**Problem**: "BRANCHES" is git jargon. All-caps + semibold was visually aggressive. Users don't understand worktrees.
**Change**: Renamed to "Workspaces" with medium weight. Updated empty state to say "No workspaces yet" and use friendlier language. Changed icon to `square.stack.3d.up`.
**Files**: `Maestro/Maestro/Views/WorktreeSidebar.swift`

## Sidebar: Add icons to stage badges
**Problem**: Stage badges (design, implement, review, polish) used color alone. Accessibility failure for color-blind users.
**Change**: Added icons to each stage badge: lightbulb (design), hammer (implement), magnifyingglass (review), sparkles (polish).
**Files**: `Maestro/Maestro/Views/WorktreeSidebar.swift`

## Prompt Input: Improve task descriptions
**Problem**: Task descriptions truncated to 1 line in dropdown. Valuable context was cut off.
**Change**: Increased line limit to 2, improved font to `.caption` from `.caption2`, added `fixedSize` for proper wrapping.
**Files**: `Maestro/Maestro/Views/PromptLauncher.swift`

## Options: Better collapse indicator
**Problem**: "Options" toggle didn't explain what was hidden when collapsed.
**Change**: Changed label from "Options" to "More options" when collapsed. Added tooltip explaining "Model, voice, context toggles, and command preview".
**Files**: `Maestro/Maestro/Views/PromptLauncher.swift`

## Toolbar: Remove redundant repo name
**Problem**: Repo name appeared in both `.navigationTitle()` and as toolbar text item. Redundant.
**Change**: Removed the toolbar text item, keeping only the navigation title.
**Files**: `Maestro/Maestro/Views/ContentView.swift`

## Welcome: Concrete tagline
**Problem**: "AI coding assistant for your projects" was abstract. Told users nothing about what actually happens.
**Change**: Changed to "Tell it what to build. It writes the code." - concrete, action-oriented. Updated icon to `wand.and.sparkles`.
**Files**: `Maestro/Maestro/Views/WelcomeWindow.swift`

## Remaining
Issues identified but not yet fixed:

- [ ] Results appear in external terminal, not in-app (high complexity - requires embedded PTY)
- [ ] No onboarding flow for first-time users (needs new OnboardingView.swift)
- [ ] Context chips all visible by default (needs progressive disclosure)
- [ ] Command preview hidden by default (power users want transparency)
- [ ] No slash commands for task selection (/design, /review)
- [ ] No @ mentions for context (@file.ts)
- [ ] No Cmd+K command palette
- [ ] No notification when background task completes
- [ ] Hover actions crowd four icons (could reduce to 2 + context menu)
- [ ] Running state pulsing dot may be missed by motion-sensitive users
