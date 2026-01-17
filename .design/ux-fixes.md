# UX Fixes Applied

## Setup: Add progress indicator

**Problem**: First-run setup showed dependencies but gave no sense of progress through the installation steps.

**Change**: Added a 3-step progress indicator (circles connected by lines) that shows:
- Current step highlighted with accent color
- Completed steps filled
- Installing state with partial opacity
- Failed state in red

**Files**: `Maestro/Maestro/Views/SetupView.swift`

## Empty State: Improve worktree guidance

**Problem**: Empty worktree state just said "No worktrees" with minimal guidance.

**Change**: Redesigned empty state with:
- Visual icon (branch symbol)
- Clearer "No worktrees yet" heading
- Explanatory text about what worktrees do
- Prominent "Create Worktree" button

**Files**: `Maestro/Maestro/Views/WorktreeSidebar.swift`

## Prompt Input: Add placeholder examples

**Problem**: The prompt input placeholder "What do you want to build?" didn't show users what kind of input is expected.

**Change**: Enhanced placeholder with:
- Main text: "Describe what you want to build..."
- Example line in smaller text: `e.g. "add user authentication" or "fix the login bug"`

**Files**: `Maestro/Maestro/Views/PromptLauncher.swift`

## Task Selector: Improve placeholder

**Problem**: Task selector showed "None" as placeholder, which is ambiguous.

**Change**: Changed placeholder to "Select task..." which is clearer about the expected action.

**Files**: `Maestro/Maestro/Views/PromptLauncher.swift`

## Error States: Add recovery hints

**Problem**: Error messages during setup showed technical errors without guidance on how to fix them.

**Change**: Improved error messages to include:
- Clear description of what failed
- Specific terminal commands to try manually
- Instruction to click Retry after manual fix

Examples:
- "Installation completed but lf not found." now includes: "Try opening Terminal and running: `pip install loopflow`"
- Failed install errors now remind users to check prerequisites

**Files**: `Maestro/Maestro/Views/SetupView.swift`

---

## Remaining

Issues identified but not yet fixed:

- [ ] Add keyboard shortcut hints in context menus
- [ ] Show loading state when creating worktrees
- [ ] Add tooltip explaining token count meaning
- [ ] Improve diff viewer with file navigation
- [ ] Add confirmation when closing window with running task
- [ ] Voice selector could show preview of voice content on hover
