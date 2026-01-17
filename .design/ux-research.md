# UX Research

## Screenshots Captured

Note: Screenshots could not be interactively captured in auto mode. Screenshot capture is available via:
- Debug menu: "Capture for Review"
- Keyboard shortcut: Cmd+4
- Saves to `.design/screenshots/` with timestamp

Key states to capture manually:
- `.design/screenshots/welcome.png` - Welcome window with recent repos
- `.design/screenshots/setup.png` - First-run dependency installation with progress stepper
- `.design/screenshots/empty-repo.png` - Repo with no worktrees (shows improved empty state)
- `.design/screenshots/prompt-launcher.png` - Main prompt input interface
- `.design/screenshots/task-dropdown.png` - Task selector with descriptions visible
- `.design/screenshots/worktree-list.png` - Sidebar with multiple worktrees
- `.design/screenshots/running-task.png` - Output panel during task execution
- `.design/screenshots/context-options.png` - Expanded context bar with chips

## Visual Issues

### Typography and Hierarchy

- [x] **Task selector label too subtle**: Fixed - task selector is now a typeahead field with clearer "Select task..." placeholder (PromptLauncher.swift:129)
- [x] **"None" placeholder ambiguous**: Fixed - changed to "Select task..." (PromptLauncher.swift:129)
- [x] **Mode toggle lacks explanation**: Fixed - tooltip added explaining Auto vs Interactive (PromptLauncher.swift:577-582)
- [x] **Token count cryptic**: Fixed - document icon and tooltip added (PromptLauncher.swift:587-600)

### Spacing and Alignment

- [ ] **Advanced options animation jarring**: Options appear/disappear with 0.2s animation but content below doesn't reflow smoothly (PromptLauncher.swift:592-595)
- [ ] **Context bar crowded with many files**: When multiple files are attached, horizontal scrolling isn't supported—chips may overflow (PromptLauncher.swift:650-729)
- [ ] **Worktree sidebar header inconsistent**: "WORKTREES" header uses 16px horizontal padding while list uses 8px (WorktreeSidebar.swift:163-165, 241)

### Color and Contrast

- [ ] **Stage badges use opacity-based colors**: `stageColor(task).opacity(0.2)` may have insufficient contrast in light mode (WorktreeSidebar.swift:650-651)
- [ ] **Tertiary text hard to read**: Multiple elements use `.foregroundStyle(.tertiary)` which can be very faint (WelcomeWindow.swift:46, PromptLauncher.swift:428)
- [ ] **Green checkmark on light background**: Worktree clean state uses `.foregroundStyle(.green)` which may not meet contrast requirements (WorktreeSidebar.swift:639)

### Unclear Affordances

- [ ] **Main input looks like display, not input**: The TextEditor has minimal visual chrome—no border until focused, though placeholder examples now help (PromptLauncher.swift:478-491)
- [ ] **Hover actions hidden by default**: Worktree row actions (diff, PR, terminal, IDE) only appear on hover—users may never discover them (WorktreeSidebar.swift:485-489)
- [ ] **Plus button for files tiny**: 10pt plus icon with 6pt padding is a small touch target (ContextChip.swift:64-71)
- [ ] **Task dropdown z-index issue**: Dropdown appears with `zIndex(100)` but may still be clipped at window edges (PromptLauncher.swift:202)

### macOS Convention Violations

- [ ] **No standard menu bar File > Open**: Opening repos requires clicking "Open Folder" button in welcome window (WelcomeWindow.swift:70-76)
- [ ] **Window title duplicates toolbar**: Repo name appears in both navigation title and toolbar item (ContentView.swift:66, 78-82)
- [x] **Setup flow not skippable**: Fixed - "Skip, I'll install manually" link added (SetupView.swift)

### Empty States

- [x] **Empty worktree message unhelpful**: Fixed - redesigned with icon, heading, explanatory text, and "Create Worktree" button (WorktreeSidebar.swift:167-193)
- [x] **Empty voices message cryptic**: Fixed - now shows "No voices configured" with explanation (PromptLauncher.swift:381-393)

## User Profile Findings

### New Developer

**First impression (0-5 seconds)**:
User sees WelcomeWindow with "Loopflow Maestro" title and subtitle "Manage worktrees and launch LLM coding sessions." The branch icon (arrow.triangle.branch) gives a git vibe.

*Reaction*: "This is something about git branches and AI? What's a worktree? What's a coding session?"

**First action**:
User clicks "Open Folder..." and selects a project directory. SetupView appears showing "First-time setup" with a 3-step progress indicator and "Loopflow CLI" / "Worktrunk (wt)" dependencies.

*Improvement*: The progress stepper now shows where you are in setup. The "Skip, I'll install manually" option gives users an escape hatch if they want to configure things themselves.

*Remaining friction*: "What is Worktrunk? Why do I need to install things? I just wanted to try the app."

**First obstacle**:
After setup completes, user sees ContentView with WorktreeSidebar. The empty state now shows:
- Branch icon
- "No worktrees yet" heading
- "Worktrees let you run tasks on isolated branches while keeping your main work untouched."
- "Create Worktree" button

*Improvement*: The explanation of worktrees is better than before, but still assumes the user knows why isolated branches matter.

The PromptLauncher shows:
- "Select task..." placeholder (clearer than "None")
- Placeholder with examples: "Describe what you want to build..." and `e.g. "add user authentication" or "fix the login bug"`
- Task dropdown now shows short descriptions extracted from task files

*Improvement*: The placeholder examples and task descriptions help users understand the expected input format.

**Recovery**:
User types something in the input and presses Run (Cmd+Enter). A worktree is auto-created with a name like "aurora-melody" (from NameGenerator). Terminal opens.

*Remaining friction*: The random poetic names (magical + musical words) are charming but confusing for new users who don't understand they're branch names.

**Verdict**: Would maybe try again, but still confused. Improvements help with immediate friction, but core concepts (worktrees, tasks, pipelines) remain unexplained. Missing:
- What is a worktree and why do I need one?
- What do these tasks actually do?
- Why did a terminal open instead of showing results in the app?

#### Pain Points (Updated)
- [ ] No explanation of core workflow: prompt -> worktree -> terminal
- [x] Task descriptions now visible in dropdown
- [x] Mode toggle now has tooltip explanation
- [ ] Random worktree names (e.g., "aurora-melody") are confusing for beginners
- [ ] No onboarding walkthrough or first-run tutorial
- [ ] Output happens in external terminal, not in-app

---

### Claude Code Power User

**First impression (0-5 seconds)**:
User sees a clean native macOS app with a familiar sidebar-detail pattern. Recognizes the Task selector with prompt names. Notices token count display with document icon.

*Reaction*: "Okay, this is a GUI for `lf` commands. Task selector = prompt files. Token count = context estimation. The doc icon and tooltip clarify what that number means."

**First action**:
User clicks Task dropdown, sees familiar task names (implement, review, design). The dropdown now shows:
- Task name with mode badge (auto/interactive)
- Short description extracted from task file content

*Improvement*: Can now see what each task does before selecting it. This matches expectations from CLI where you'd read the task file.

User selects "implement", types args in the main input. Hovers over mode picker and sees tooltip: "Auto: runs to completion without input. Interactive: opens a chat session you can guide."

**First obstacle**:
User wants to run with specific flags (`-m codex`, `--voice architect`) but finds:
- Voice selector is accessible under "Options" toggle
- Model selection is still not visible in the UI
- No way to pass custom flags or see the command being built

*Frustration*: "I can see voices now, but where's the model selector? I want to use `-m codex:o3`. And I still can't see what command will actually run."

**Recovery**:
User clicks "Options" to reveal Voice selector and Context bar. Finds voices with improved empty state message. Still no model picker.

Power user might open terminal and use `lf` directly for full control. The GUI is useful for:
- Visual worktree management
- Quick launches with common settings
- Tracking multiple running sessions via OutputPanel

**Verdict**: More useful than before for quick operations. Still falls back to CLI for:
- Model selection (critical gap)
- Custom flags
- Command preview/debugging
- Parallel model execution

The buildCommand() function in AppState.swift (lines 291-366) shows the command is being built—just not displayed to the user.

#### Pain Points (Updated)
- [ ] **No model selector visible** - still missing, critical for power users
- [ ] **No command preview** - buildCommand() exists but isn't shown before execution
- [x] Task descriptions now visible in dropdown
- [x] Token count has icon and tooltip
- [x] Mode toggle has tooltip explanation
- [ ] No `--parallel` model racing support visible
- [ ] Pipeline editor hidden behind beta flag

---

### Designer/PM

**First impression (0-5 seconds)**:
User sees WelcomeWindow. Clean design, subtle colors. "Manage worktrees" is confusing—sounds like forestry. Clicks through.

*Reaction*: "Install Loopflow? Worktrunk? What are these? I just wanted to try AI coding help."

*Improvement*: Setup now has skip option and clearer error recovery messages. But the dependency names are still jargon.

**First action**:
After setup, user sees the main interface. The placeholder now says "Describe what you want to build..." with examples like "add user authentication" or "fix the login bug".

*Improvement*: The examples are developer-focused but give some idea of the input format.

User types "help me write a product spec for user authentication" and looks at Task dropdown. Sees tasks like:
- "design" - "Produce a short implementation spec that another LLM session can use..."
- "implement" - "Turn the design doc into working code."
- "review" - "Review the diff on this branch..."

*Partial improvement*: Descriptions help, but they're still developer-focused. No clear "write docs" or "brainstorm" option.

**First obstacle**:
User hits Run. A terminal window opens with scrolling text. It's running `claude` commands.

*Confusion points*:
- "Why did a black terminal window open?"
- "What is all this scrolling text?"
- "How do I talk to it? Where do I type?"

**Recovery**:
User might try "Interactive" mode (tooltip now explains it). The terminal becomes interactive, but it's still a terminal—not a chat UI the PM would expect from ChatGPT or Notion AI.

**Verdict**: Would not come back. The app assumes familiarity with:
- Git and branches (worktrees are git concepts)
- Command-line interfaces (output is in terminal)
- Developer workflows (tasks are code-focused)

The core interaction model—launching external terminal processes—is fundamentally intimidating for non-technical users.

#### Pain Points (Updated)
- [ ] Jargon-heavy: "worktrees", "prompts", "pipelines", "voices"
- [x] Task descriptions now visible, but still developer-focused
- [ ] Terminal output is frightening for non-technical users
- [ ] No visual feedback in the app itself—everything happens in external terminal
- [ ] No templates or examples for non-code tasks (docs, specs, planning)
- [ ] Core value proposition unclear for non-developers

---

## Top 5 Priority Issues

### 1. **No model selector in UI** (Power User Blocker)
CLI's `-m` flag has no GUI equivalent. Power users can't select claude:opus vs codex:o3 from the UI. This forces them back to the CLI for any serious work.
- **Impact**: High - power users are the primary audience
- **Fix**: Add model picker to the options bar or as a dropdown next to the Run button

### 2. **No command preview before execution** (Trust & Learning)
The buildCommand() function exists (AppState.swift:291-366) but the assembled command isn't shown to users. They can't:
- Learn the CLI by seeing what the GUI generates
- Debug when things go wrong
- Verify the right options are set
- **Impact**: High - prevents learning and debugging
- **Fix**: Show a collapsible "Command preview" row showing the `lf` command

### 3. **Output happens in external terminal** (Friction for All Users)
Every run launches Warp/Terminal externally. This:
- Breaks the user's focus
- Requires terminal familiarity
- Loses the spatial connection between input and output
- **Impact**: High - affects all personas
- **Fix**: Consider inline output option (OutputPanel exists but only shows daemon events, not the actual task output)

### 4. **No onboarding or workflow explanation** (New User Blocker)
Users land in the main UI without understanding:
- What worktrees are and why they matter
- What each task does (descriptions help but aren't enough)
- The prompt -> worktree -> terminal workflow
- **Impact**: Medium-High - new users bounce immediately
- **Fix**: First-run walkthrough or contextual tooltips on key elements

### 5. **Random worktree names are confusing** (New User Confusion)
NameGenerator creates poetic names like "aurora-melody" which are:
- Charming for power users who get the convention
- Confusing for new users who don't realize these are branch names
- **Impact**: Medium - adds cognitive load during first run
- **Fix**: Ask for a worktree name or derive from prompt ("auth-feature" from "add user authentication")

---

## Issues Fixed Since Last Research

These issues from previous research have been addressed:

1. **Task selector placeholder "None" was ambiguous** -> Now "Select task..."
2. **Mode toggle had no explanation** -> Tooltip added
3. **Token count was cryptic** -> Icon and tooltip added
4. **Empty worktree state was unhelpful** -> Redesigned with explanation
5. **Empty voices message assumed file system knowledge** -> Better explanation
6. **Setup flow was not skippable** -> "Skip, I'll install manually" added
7. **Task dropdown showed no descriptions** -> Now extracts first content line
8. **Error messages had no recovery guidance** -> Terminal commands suggested

## Remaining Visual/Interaction Issues

- [ ] Hover actions on worktree rows are hidden until hover (discoverability)
- [ ] Plus button for adding files is tiny (6pt padding)
- [ ] Context bar doesn't scroll when many files attached
- [ ] Stage badges may have insufficient contrast in light mode
- [ ] Tertiary text (.foregroundStyle(.tertiary)) is too faint
- [ ] Window title duplicates toolbar item
- [ ] No File > Open in menu bar
