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

## Context Preview Panel

**Problem**: Users can't see what context is being assembled. Token count is a single number with no breakdown. No way to preview the assembled prompt (CLI's `-c` flag has no GUI equivalent).

**Change**: Implemented expandable context preview panel:
- Token count is now clickable → expands preview panel below
- Preview shows sections: Docs, Files, Diff, Clipboard, Attached
- Each section shows item-level breakdown with token counts
- Files and attached items can be removed via ✕ button
- Copy button exports full assembled context to clipboard
- Toggles real-time update the preview

**Files**:
- `Maestro/Maestro/Views/PromptLauncher.swift` (UI)
- `Maestro/Maestro/Services/ContextPreviewService.swift` (data)
- `Maestro/Maestro/Models/ContextPreview.swift` (models)
- `Maestro/Maestro/AppState.swift` (state management)

---

## Task Typeahead Search

**Problem**: Task selector required knowing task names upfront or scrolling through a list.

**Change**: Task selector now has searchable typeahead:
- Type to filter tasks
- Keyboard navigation (↑/↓) in dropdown
- Shows both tasks and pipelines (pipelines in separate section)
- Mode badges visible in dropdown

**Files**: `Maestro/Maestro/Views/PromptLauncher.swift`

---

## Running State Indicator

**Problem**: Worktree rows don't show when a task is actively running. Research finding: "Running state not visible on worktree rows—no spinner or indicator."

**Change**: Worktree rows now show a pulsing blue dot when a session is running in that worktree:
- Session events include worktree path for tracking
- AppState tracks `activeWorktreePaths` set
- WorktreeRow shows animated pulsing indicator when `isRunning` is true
- Animation uses scale and opacity for subtle attention-grabbing effect

**Files**:
- `src/loopflow/lfd/server.py` (added worktree to session.started event)
- `Maestro/Maestro/Services/LFDEventService.swift` (parse worktree from event)
- `Maestro/Maestro/AppState.swift` (track active worktree paths)
- `Maestro/Maestro/Views/WorktreeSidebar.swift` (pulsing indicator in WorktreeRow)

---

## Keyboard Shortcut: Focus Prompt

**Problem**: No keyboard shortcut to quickly focus the prompt input. Research finding: "No Cmd+K command palette—keyboard navigation slower than CLI."

**Change**: Added Cmd+L keyboard shortcut to focus the prompt input:
- Cmd+L focuses the main text editor in PromptLauncher
- Menu item in Edit menu for discoverability
- Consistent with browser URL bar pattern

**Files**:
- `Maestro/Maestro/Views/PromptLauncher.swift` (hidden button with shortcut)
- `Maestro/Maestro/MaestroApp.swift` (menu item for discoverability)

---

## Remaining

Issues identified but not yet fixed:

### High Priority

- [ ] **Dual input confusion**: Task selector + colon syntax in text field still compete. Proposal: Replace with single input using `/` prefix (Notion-style).
- [ ] **Full command palette**: Cmd+K for all actions (current: only Cmd+L for prompt focus)

### Medium Priority

- [ ] **Mental model gap**: Users expect in-app responses; Maestro launches terminal sessions. Proposal: Transform OutputPanel into results summary view.
- [ ] **Output panel redundant**: Streaming panel duplicates terminal output. Should become results view showing "what changed."
- [ ] **Sidebar keyboard nav**: Arrow keys to navigate worktrees, Enter to select

### Lower Priority

- [ ] **Silent dependency install**: Setup still uses blocking wizard instead of background install
- [ ] **Template gallery**: No starter tasks for repos without `.lf/` config
- [ ] **Worktree auto-creation message**: No notification when auto-creating from main
- [ ] **@ mentions**: No way to add specific files via typing `@filename` in prompt
- [ ] **Inline validation**: Branch name availability not checked while typing
