# UX Fixes Applied

Based on research from `.design/ux-research.md` and `.design/ux-gaps.md`.

---

## First-Run: Remove jargon from welcome screen

**Problem**: Welcome screen uses insider terminology ("worktrees", "LLM coding sessions") that confuses beginners. Research finding: "The terminology assumes familiarity. 'Worktrees' and 'LLM coding sessions' are insider jargon."

**Change**: Replaced subtitle "Manage worktrees and launch LLM coding sessions" with "AI coding assistant for your projects"

**Files**: `Maestro/Maestro/Views/WelcomeWindow.swift`

---

## First-Run: Explain dependencies during setup

**Problem**: SetupView shows technical names ("Loopflow CLI", "Worktrunk") without explaining what they enable. Research finding: "The descriptions ('Core command-line tool', 'Git worktree manager') are accurate but not helpful. They don't explain why these are needed."

**Change**: Updated dependency descriptions to explain benefits:
- "Loopflow CLI" → "Runs AI coding tasks from your terminal"
- "Worktrunk (wt)" → "Keeps each feature in its own folder"

**Files**: `Maestro/Maestro/Views/SetupView.swift`

---

## Empty State: Explain worktrees when sidebar is empty

**Problem**: Empty worktree state says "No worktrees / Click + to create one" without explaining what worktrees are. Research finding: "First-time users don't understand why their request created a folder called 'electric-penguin'."

**Change**: Added explanatory empty state with:
- Visual icon (branch symbol)
- Heading "No worktrees yet"
- Explanation: "Each worktree is an isolated folder where AI can work without affecting your main code."
- Prominent "Create Worktree" button

**Files**: `Maestro/Maestro/Views/WorktreeSidebar.swift`

---

## Sidebar: Less jargon in header

**Problem**: "WORKTREES" header uses git-specific terminology. Research finding: "'Worktrees' appears without explanation."

**Change**: Changed header from "WORKTREES" to "BRANCHES" with tooltip "Worktrees: isolated folders for each feature branch". Changed "New Worktree" tooltip to "Create a new branch in its own folder".

**Files**: `Maestro/Maestro/Views/WorktreeSidebar.swift`

---

## Mode Picker: Add tooltip explaining options

**Problem**: Mode picker shows "Auto" vs "Interactive" without explaining the difference. Research finding: "Mode picker asks users to choose Auto vs Interactive without explaining implications."

**Change**: Added dynamic tooltip that explains the currently selected mode:
- Auto: "Runs to completion without interruption"
- Interactive: "Chat with the AI, can interrupt and redirect"

**Files**: `Maestro/Maestro/Views/PromptLauncher.swift`

---

## Prompt Input: More inviting placeholder

**Problem**: "What do you want to build?" placeholder may be unclear. Research finding: "The 'What do you want to build?' prompt is intimidating."

**Change**: Changed placeholder to "Describe what you want to build or change..."

**Files**: `Maestro/Maestro/Views/PromptLauncher.swift`

---

## Errors: More actionable error messages

**Problem**: Error messages are technical and don't suggest recovery paths. Research finding: "Error messages are technical, not actionable... Every error should suggest at least one path forward."

**Changes**:
1. WorktreeService errors now provide context:
   - "branch already exists" → "A branch with this name already exists. Try a different name."
   - Generic command failures show the actual error without "Worktree command failed:" prefix
   - "wt not installed" → "Worktrunk not installed. Click retry to install it."

2. PromptLauncher error dialog:
   - Title changed from "Launch Failed" to "Couldn't Start"
   - More helpful fallback: "Something went wrong. Try again."
   - Worktree not found: "Created the branch but couldn't find it. Try refreshing the sidebar."

**Files**:
- `Maestro/Maestro/Services/WorktreeService.swift`
- `Maestro/Maestro/Views/PromptLauncher.swift`

---

## Remaining

Issues identified but not yet fixed:

### High Priority

- [ ] **Dual input confusion**: Task selector + colon syntax in text field compete; prompt picker interrupts typing
- [ ] **Context opacity**: Users can't see what context is being assembled; toggle blindly
- [ ] **No keyboard-first navigation**: Missing Cmd+K command palette

### Medium Priority

- [ ] **Mental model gap**: Users expect in-app responses; Maestro launches terminal sessions
- [ ] **Output panel value**: Streaming output panel duplicates terminal output but offers no unique value
- [ ] **Token count meaningless**: Single number with no breakdown

### Lower Priority

- [ ] **No onboarding flow**: No first-run guidance or progressive disclosure of concepts
- [ ] **Worktree auto-creation message**: When auto-creating, show brief message explaining what happened
- [ ] **Running state invisible**: Worktree rows don't show active sessions
