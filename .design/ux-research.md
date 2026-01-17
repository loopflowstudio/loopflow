# UX Research

## Screenshots Captured
- `.design/screenshots/maestro-main.png` - Main interface with repo open (system dialog for screen recording permission overlays the view)

**Note**: The captured screenshot shows a macOS "Screen & System Audio Recording" permission dialog blocking most of the main interface. The dialog requests access for "MaestroU0.2026.01.14.08.16.sta-9kc_042" - an auto-generated identifier that looks suspicious to users. This reveals a first-run friction point: users attempting to use debug capture features immediately encounter permission dialogs with cryptic app identifiers. Analysis below combines the visible screenshot elements with comprehensive code review of all SwiftUI views.

## Visual Issues

### Layout and Spacing
- [x] **Task selector horizontal layout** - Fixed: Task selector now uses typeahead field with inline label
- [ ] **Context bar horizontal overflow** (`PromptLauncher.swift:947-958`): Five chips (Docs, Files, Diff, Clipboard, Summaries) shown immediately with no max-width constraint - overwhelming visual density
- [ ] **Worktree empty state centering** (`WorktreeSidebar.swift:168-196`): Content floats low in tall windows due to `maxHeight: .infinity`
- [ ] **Result panel header density** (`ResultsPanel.swift:64-120`): Five controls compete for attention in one row (status, text, duration, toggle, clear, expand)
- [ ] **Options section visual separation** (`PromptLauncher.swift:60-66`): Model, voice, context, and command preview lack clear grouping
- [ ] **Permission dialog app identifier** (screenshot): System dialog shows "MaestroU0.2026.01.14.08.16.sta-9kc_042" - confusing

### Typography Hierarchy
- [x] **Sidebar header weight** - Fixed: Changed from "BRANCHES" (all-caps semibold) to "Workspaces" (medium weight title case)
- [x] **Placeholder text prominence** - Fixed: Changed to "What should the AI build?" at `.secondary` with example text
- [x] **Task dropdown descriptions** - Fixed: Increased from 1-line to 2-line limit with `.caption` font
- [ ] **Mixed caption sizing**: Inconsistent use of `.caption` vs `.caption2` across views without clear system

### Color and Contrast
- [x] **Stage badge accessibility** - Fixed: Icons added (lightbulb, hammer, magnifyingglass, sparkles) alongside colors
- [ ] **Disabled state opacity** (`PromptLauncher.swift:886`): Context sections at `opacity(0.5)` may not meet WCAG 4.5:1 contrast
- [ ] **Running state indicator** (`WorktreeRow.swift:659-664`): Pulsing blue dot relies on animation - accessibility concern for motion-sensitive users
- [ ] **Selected worktree** (`WorktreeRow.swift:527-530`): Blue accent at 15% opacity may be too subtle

### Visual Clutter
- [ ] **Context chips all visible** (`PromptLauncher.swift:947-958`): Docs, Files, Diff, Clipboard, Summaries shown immediately - no progressive disclosure
- [ ] **Hover action overload** (`WorktreeRow.swift:593-647`): Four icons (diff, PR, terminal, IDE) on hover - all similar size/style
- [ ] **Token count in crowded row** (`PromptLauncher.swift:740-756`): Embedded with mode picker and options button

### macOS Platform Conventions
- [x] **Repo name duplication** - Fixed: Removed redundant toolbar text item, keeping only navigation title
- [ ] **Permission dialog confusion** (screenshot): System dialog shows gibberish app identifier
- [ ] **Sheet sizing** (`NewWorktreeSheet.swift:756`): Fixed 320pt width feels cramped on larger displays
- [ ] **No File > Open Recent**: Recent repos only in welcome window, not accessible while working

## User Profile Findings

### New Developer

**First impression** (0-5 seconds):
User opens Maestro and sees a split-panel interface. The left sidebar shows "Workspaces" with an empty state message: "No workspaces yet" and "Create a workspace to let AI work on a feature without affecting your main code." This is clearer than the previous "BRANCHES" terminology.

The main area shows a text field with "What should the AI build?" placeholder and helpful example: "Try: 'add user authentication' or 'refactor the API to use async/await'". This is concrete and inviting. Above it, the Task selector shows "implement" pre-selected - a sensible default.

However, below the input they see:
- Mode picker: "Auto" / "Interactive" with no explanation
- Token count "14.2k tokens" - meaningless to newcomers
- "More options" toggle hiding model/voice/context controls
- Five colored chips: Docs, Files, Diff, Clipboard, Summaries - developer jargon

**First action**:
They type "add a login button to the homepage" in the text area. The pre-selected "implement" task seems right. They notice the Run button with "⌘↵" shortcut - standard macOS pattern.

**First obstacle**:
They press Run (⌘↵). Two scenarios:

1. **If no worktree selected**: A worktree is auto-created with a generated name like "floral-tiger". They see this appear in the sidebar but have no idea what it means or where the work is happening.

2. **When task starts**: A terminal window opens (Warp or Terminal) with Claude Code output. Maestro shows "Running implement..." in the results panel but the actual work happens in the external terminal. They must context-switch to see progress.

The disconnect is jarring: they launched from Maestro but must watch Terminal. If they lose the terminal window, they have no visibility into what's happening. The results panel shows status but not live output.

**Recovery**:
After completion, the results panel shows "implement completed" with file changes expandable. The "View Full Diff" button is useful. But the workflow broke their flow - they expected results in-app.

**Verdict**:
**Mixed experience.** The prompt input is much improved - concrete examples, sensible defaults, task pre-selected. But the terminal context-switch is disorienting. They'd come back but feel the app is "a launcher" rather than a complete workflow tool.

#### Pain Points
- [ ] Results stream to external terminal, not in-app - breaks flow
- [ ] Auto-generated worktree names ("floral-tiger") are cute but confusing
- [ ] "Auto" vs "Interactive" mode picker has no explanation
- [ ] Token count assumes LLM knowledge
- [ ] Context chips use developer jargon without tooltips for new users
- [ ] No indication of what happens before pressing Run

### Claude Code Power User

**First impression** (0-5 seconds):
Immediately recognizes this as loopflow GUI. Task selector shows available prompts - "oh, this is `lf <task>`". The pre-selected "implement" task is smart. Token count makes sense. Context chips (Docs, Files, Diff, etc.) map directly to CLI flags. "More options" reveals model selector, voice selector - exactly the CLI flags.

Sidebar shows "Workspaces" with familiar worktree information: branch names, commit counts, stage badges (design/implement/review/polish). The icons on stage badges are a nice accessibility touch.

**First action**:
Select a worktree, type a prompt, check "More options" to see model and voice selectors. Look for command preview to verify what will run.

**First obstacle**:
Command preview exists (`PromptLauncher.swift:1050-1095`) but collapsed by default. They want to:
- See the exact command before running
- Add custom flags like `--parallel`
- Use a model not in the dropdown

The model selector shows common models but power users may need `claude:haiku` or custom configurations. No way to enter arbitrary CLI flags.

When they run, output streams to external terminal. They expected this (it's how Claude Code works), but they'd prefer in-app streaming to avoid context switching. The results panel after completion is good - shows file changes with expandable diffs.

**Recovery**:
They copy the command from the preview and run it in terminal for more control. The app becomes a "visual launcher" rather than complete workflow tool.

**Verdict**:
**Would use for specific features.** Main value: visual worktree status, quick launching, PR management, diff comparison. Missing: live in-app output, arbitrary CLI flags, full model list. They'd use Maestro alongside CLI, not instead of it.

#### Pain Points
- [ ] Command preview collapsed by default - should be prominent
- [ ] No custom CLI flags (`--parallel`, `--no-diff`, etc.)
- [ ] Live output streams to terminal, not in-app
- [ ] Model selector shows only common models
- [ ] Can't create/edit voice files inline
- [ ] No diff preview before running
- [ ] Pipelines/Agents behind `Flags.beta` with no discovery path

### Designer/PM

**First impression** (0-5 seconds):
Clean macOS app with familiar split-view. The left sidebar says "Workspaces" with a friendly empty state. The main area has a large text field with "What should the AI build?" - very inviting, feels like ChatGPT.

Below they see:
- "Task" selector showing "implement"
- "Auto" / "Interactive" toggle - unclear
- Colorful chips: Docs (blue), Files (teal), Diff (green), Clipboard (purple)
- Token count "14.2k" - what does that mean?

The welcome screen (if they saw it) said "Tell it what to build. It writes the code." - concrete and good.

**First action**:
Type "write a summary of what this project does" - they want documentation, not code. They're unsure if "implement" is the right task. Maybe "design"? They don't understand the difference.

**First obstacle**:
Multiple friction points stack up:

1. **Task selection confusion**: The dropdown shows tasks with 2-line descriptions, but "implement: Turn design doc into working code" doesn't match their need (documentation).

2. **Worktree creation**: When they run, a worktree named "sunny-koala" appears in sidebar. They don't understand what a worktree is, despite the empty state explanation.

3. **Terminal launch**: Warp or Terminal opens with scrolling text - foreign territory for non-technical users. They didn't expect this.

4. **Technical output**: The terminal shows file paths, git commands, agent status. Even with Claude Code's friendly arrows (→), it's developer-speak.

5. **Results panel**: "3 files changed, +45 -12" - meaningless without developer context.

**Recovery**:
They need someone to explain:
- That tasks are for code changes, not documentation
- That the terminal is where the work happens
- What a worktree is and why they need one

The app doesn't provide this scaffolding in-context. The onboarding is missing.

**Verdict**:
**Would not return without guidance.** The prompt input is welcoming, but everything after pressing Run assumes developer mental models. Git terminology (even "workspace" is slightly confusing), terminal output, technical results summary. A PM writing specs or designer prototyping UI would be lost.

#### Pain Points
- [ ] No task for "explain" or "document" - all tasks assume code changes
- [ ] Task descriptions don't help non-developers choose
- [ ] Terminal launch is unexpected and intimidating
- [ ] Git concepts (worktree, branch, diff, commit) assumed known
- [ ] No "simple mode" hiding technical complexity
- [ ] Results panel assumes familiarity with line counts and diffs
- [ ] Token count displayed but never explained
- [ ] No onboarding flow explaining workflow

## Top 5 Priority Issues

1. **Results appear in external terminal, not in-app**
   - Users launch from Maestro but must watch Terminal for progress
   - Location: `PromptLauncher.swift` launches via `TerminalLauncher`
   - Impact: All profiles experience context-switch; Designer/PM finds Terminal intimidating
   - Fix: Embed terminal via SwiftTerm (see `.design/embedded-terminal.md`)

2. **No onboarding flow for first-time users**
   - Full interface shown immediately with no guided introduction
   - Location: App entry point - `RepoWindow.swift` shows `SetupView` for dependencies only
   - Impact: All profiles struggle with concepts (worktrees, tasks, modes)
   - Fix: Add 3-4 step walkthrough: "What Maestro does → How workspaces work → Your first task → Where to see results"

3. **Context controls visible but unexplained**
   - Five chips (Docs, Files, Diff, Clipboard, Summaries) shown without explanation
   - Location: `PromptLauncher.swift:947-958` contextBar section
   - Impact: New Developer confused; Designer/PM overwhelmed; Power User wants them visible
   - Fix: Progressive disclosure - collapse to "Context: 14.2k" by default, expand to show toggles

4. **"Auto" vs "Interactive" mode picker meaningless**
   - Two-option picker with no explanation of what each mode does
   - Location: `PromptLauncher.swift` mode picker
   - Impact: All profiles don't understand the distinction until they try both
   - Fix: Add hover tooltips - Auto: "Runs to completion" / Interactive: "Chat with the AI, can redirect"

5. **Command preview hidden when power users want transparency**
   - Power users want to see exactly what will run before executing
   - Location: `PromptLauncher.swift:1050-1095` collapsed by default
   - Impact: Power users copy command to terminal for confidence
   - Fix: Show command preview by default, or make toggle more prominent

## Additional Observations from Code Review

### Improvements Since Last Research (Already Applied)
- [x] **"BRANCHES" → "Workspaces"**: Friendlier terminology (WorktreeSidebar.swift:147-149)
- [x] **Stage badges with icons**: Accessibility improvement - lightbulb/hammer/magnifyingglass/sparkles
- [x] **Task pre-selected to "implement"**: Smart default saves decision
- [x] **Better placeholder text**: "What should the AI build?" with concrete examples
- [x] **2-line task descriptions**: Increased from 1-line truncation
- [x] **"More options" label**: Better than generic "Options" when collapsed
- [x] **Welcome tagline**: "Tell it what to build. It writes the code."
- [x] **Removed redundant repo name**: Was in both title and toolbar

### Positive Patterns
- **SetupView** has clear 3-step progress indicator with helpful descriptions - good model for onboarding
- **WelcomeWindow** recent repos list is fast and useful for returning users
- **Task dropdown** shows mode badge (auto/interactive) - good information density
- **Keyboard shortcuts** standard macOS (Cmd+Enter to run, Cmd+L to focus prompt)
- **DiffSheet/CompareSheet** excellent diff visualization with syntax highlighting
- **ResultsPanel** file changes expandable with diff preview - good progressive disclosure
- **Context preview panel** (when expanded) shows exactly what's being sent - great transparency

### Missing Affordances
- No "?" help buttons or contextual help anywhere in main interface
- No links to documentation or getting-started guide
- Tooltips inconsistent - some `.help()` modifiers present, many missing
- No undo/cancel for running tasks (must close terminal)
- No notification when background task completes (requires watching terminal)
- No Cmd+K command palette (mentioned in DESIGN.md principles but not implemented)

### Technical Debt Affecting UX
- **Flags.beta** hides Pipelines/Agents with no UI discovery path
- **TerminalLauncher** only output path - no in-app fallback
- **NameGenerator** whimsical names - delightful for power users, confusing for newcomers
- **Session tracking requires lfd daemon** - if not running, features silently degrade
- **Screen capture permission** shows cryptic app identifier in system dialog

### Design Principle Alignment
Comparing against `Maestro/DESIGN.md`:

| Principle | Current State | Status |
|-----------|---------------|--------|
| Immediate Connection (Bret Victor) | Output streams to external terminal | Gap |
| Progressive Disclosure (Notion) | "More options" helps, but context chips visible | Partial |
| Speed as Feature (Linear) | UI responsive, launches quickly | Good |
| Keyboard-First (Linear) | Cmd+Enter, Cmd+L, Cmd+N work | Partial - no Cmd+K |
| Opinionated Defaults (Linear) | Task pre-selected, good context defaults | Good |
| Transparency (Cursor) | Command preview available but hidden | Partial |
| Design Should Disappear (Ive) | Clean, but context chips add visual noise | Good |
| Remove Barriers (fast.ai) | Requires git/loopflow understanding | Gap |

## Open Questions

Captured in `.design/questions.md`:

1. **In-app terminal embedding**: SwiftTerm research complete (`.design/terminal-embedding.md`). Ready to implement?
2. **Onboarding flow**: What should the 3-4 step walkthrough cover?
3. **Progressive disclosure for context**: Collapse chips by default? Show token count only until clicked?
4. **Mode explanation**: Tooltips sufficient or need inline explanation?
5. **Non-developer audience**: Should Maestro support non-code tasks (documentation, summaries)?
