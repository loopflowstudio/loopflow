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

## Mode Toggle: Add explanatory tooltip

**Problem**: "Auto" vs "Interactive" modes have no visible explanation; users must guess what these mean.

**Change**: Added tooltip on the mode segmented picker explaining:
- Auto: runs to completion without input
- Interactive: opens a chat session you can guide

**Files**: `Maestro/Maestro/Views/PromptLauncher.swift`

## Token Count: Add icon and tooltip

**Problem**: Token count "14.2k" shown without label—users won't know this is estimated context tokens.

**Change**: Added:
- Document icon prefix to provide visual context
- Tooltip explaining: "Estimated context size in tokens. Includes docs, files, and other context you've enabled."

**Files**: `Maestro/Maestro/Views/PromptLauncher.swift`

## Voices: Improve empty state message

**Problem**: Empty voices message "No voices in .lf/voices/" assumes knowledge of file system structure.

**Change**: Redesigned empty state with:
- Clearer heading: "No voices configured"
- Explanatory text: "Add .md files to .lf/voices/ to create personas that shape how the agent responds."

**Files**: `Maestro/Maestro/Views/PromptLauncher.swift`

## Setup: Add skip option

**Problem**: Setup flow not skippable—users can't dismiss setup if they want to manually install dependencies.

**Change**: Added "Skip, I'll install manually" link below the main action button that allows users to proceed without automatic installation.

**Files**: `Maestro/Maestro/Views/SetupView.swift`

## Task Dropdown: Add descriptions

**Problem**: Task selector dropdown shows names but no descriptions—users can't make informed choices about what each task does.

**Change**: Added:
- `shortDescription` computed property on PromptCard that extracts the first content line (after frontmatter and headers)
- Description shown below task name in dropdown, truncated to 60 chars

**Files**:
- `Maestro/Maestro/Models/PromptCard.swift`
- `Maestro/Maestro/Views/PromptLauncher.swift`

## Model Selector: Add model picker to Options

**Problem**: CLI's `-m` flag has no GUI equivalent. Power users can't select claude:opus vs codex:o3 from the UI, forcing them back to CLI for model selection.

**Change**: Added model selector in the Options area:
- Dropdown showing "Default" option (uses config's agent_model) plus common models
- Common models: Claude, Claude Opus, Claude Sonnet, Codex, Codex O3, Codex O4-Mini, Gemini
- Selected model shown with chevron indicator
- When non-default model selected, `-m` flag added to command

**Files**:
- `Maestro/Maestro/Models/LoopflowConfig.swift` - Added `AgentModel` struct
- `Maestro/Maestro/AppState.swift` - Added `selectedModel` state and updated `buildCommand()`
- `Maestro/Maestro/Views/PromptLauncher.swift` - Added model selector UI

## Command Preview: Show what will execute

**Problem**: The buildCommand() function exists but the assembled command isn't shown to users. They can't learn the CLI by seeing what the GUI generates, debug when things go wrong, or verify the right options are set.

**Change**: Added collapsible command preview in Options area:
- "Command Preview" toggle with terminal icon
- Shows the full `lf` command that will execute
- Monospaced font, selectable text
- Copy-to-clipboard button
- Updates live as options change

**Files**:
- `Maestro/Maestro/Views/PromptLauncher.swift` - Added `commandPreview` view

---

## Remaining

Issues identified but not yet fixed:

- [ ] Add keyboard shortcut hints in context menus
- [ ] Show loading state when creating worktrees
- [ ] Improve diff viewer with file navigation
- [ ] Add confirmation when closing window with running task
- [ ] Voice selector could show preview of voice content on hover
