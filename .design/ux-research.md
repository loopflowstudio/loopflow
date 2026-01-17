# UX Research

## Screenshots Captured

Note: Screenshots could not be interactively captured in auto mode. Screenshot capture is available via:
- Debug menu: "Capture for Review"
- Keyboard shortcut: Cmd+Shift+S
- Saves to `.design/screenshots/` with timestamp

Key states to capture manually:
- `.design/screenshots/welcome.png` - Welcome window with recent repos
- `.design/screenshots/setup.png` - First-run dependency installation
- `.design/screenshots/empty-repo.png` - Repo with no worktrees
- `.design/screenshots/prompt-launcher.png` - Main prompt input interface
- `.design/screenshots/worktree-list.png` - Sidebar with multiple worktrees
- `.design/screenshots/running-task.png` - Output panel during task execution
- `.design/screenshots/context-options.png` - Expanded context bar with chips

## Visual Issues

### Typography and Hierarchy

- [ ] **Task selector label too subtle**: "Task" label at `.font(.caption).foregroundStyle(.secondary)` is easy to miss; new users may not understand this is a dropdown (PromptLauncher.swift:102-104)
- [ ] **"None" placeholder ambiguous**: Task selector shows "None" when empty—unclear if this means "no task selected" or "no tasks available" (PromptLauncher.swift:109)
- [ ] **Mode toggle lacks explanation**: "Auto" vs "Interactive" modes have no visible explanation; users must guess what these mean (PromptLauncher.swift:577-582)
- [ ] **Token count cryptic**: "14.2k" shown without label—users won't know this is estimated context tokens (PromptLauncher.swift:587-589)

### Spacing and Alignment

- [ ] **Advanced options animation jarring**: Options appear/disappear with 0.2s animation but content below doesn't reflow smoothly (PromptLauncher.swift:592-595)
- [ ] **Context bar crowded with many files**: When multiple files are attached, horizontal scrolling isn't supported—chips may overflow (PromptLauncher.swift:650-729)
- [ ] **Worktree sidebar header inconsistent**: "WORKTREES" header uses 16px horizontal padding while list uses 8px (WorktreeSidebar.swift:163-165, 241)

### Color and Contrast

- [ ] **Stage badges use opacity-based colors**: `stageColor(task).opacity(0.2)` may have insufficient contrast in light mode (WorktreeSidebar.swift:650-651)
- [ ] **Tertiary text hard to read**: Multiple elements use `.foregroundStyle(.tertiary)` which can be very faint (WelcomeWindow.swift:46, PromptLauncher.swift:428)
- [ ] **Green checkmark on light background**: Worktree clean state uses `.foregroundStyle(.green)` which may not meet contrast requirements (WorktreeSidebar.swift:639)

### Unclear Affordances

- [ ] **Main input looks like display, not input**: The TextEditor has minimal visual chrome—no border until focused (PromptLauncher.swift:522-530)
- [ ] **Hover actions hidden by default**: Worktree row actions (diff, PR, terminal, IDE) only appear on hover—users may never discover them (WorktreeSidebar.swift:485-489)
- [ ] **Plus button for files tiny**: 10pt plus icon with 6pt padding is a small touch target (ContextChip.swift:64-71)
- [ ] **Dropdown appears below, obscured**: Task selector dropdown appears at fixed y-offset and may be clipped by window bounds (PromptLauncher.swift:248)

### macOS Convention Violations

- [ ] **No standard menu bar File > Open**: Opening repos requires clicking "Open Folder" button in welcome window
- [ ] **Window title duplicates toolbar**: Repo name appears in both navigation title and toolbar item (ContentView.swift:66, 78-82)
- [ ] **Setup flow not skippable**: Users can't dismiss setup if they want to manually install dependencies

### Empty States

- [ ] **Empty worktree message unhelpful**: "No worktrees" / "Click + to create one" doesn't explain what a worktree is (WorktreeSidebar.swift:167-176)
- [ ] **Empty voices message cryptic**: "No voices in .lf/voices/" assumes knowledge of file system structure (PromptLauncher.swift:381-385)

## User Profile Findings

### New Developer

**First impression (0-5 seconds)**:
User sees WelcomeWindow with "Loopflow Maestro" title and subtitle "Manage worktrees and launch LLM coding sessions." The branch icon (arrow.triangle.branch) gives a git vibe.

*Reaction*: "This is something about git branches and AI? What's a worktree? What's a coding session?"

**First action**:
User clicks "Open Folder..." and selects a project directory. SetupView appears showing "First-time setup" with "Loopflow CLI" and "Worktrunk (wt)" dependencies.

*Reaction*: "What is Worktrunk? Why do I need to install things? I just wanted to try the app."

**First obstacle**:
After setup completes, user sees ContentView with empty WorktreeSidebar showing "No worktrees" and PromptLauncher with a big empty text field asking "What do you want to build?"

*Confusion points*:
- "What's a worktree and why do I have none?"
- "The Task dropdown shows various tasks like 'review', 'implement', 'design'—what do these do?"
- "What is 'Auto' vs 'Interactive' mode?"
- "What do Docs/Files/Diff/Clipboard toggles mean?"

**Recovery**:
User might type something in the input and press Run (Cmd+Enter). A worktree gets auto-created with a random name. Terminal opens and something happens. User has no idea what's going on.

**Verdict**: Would not come back. Too much unexplained jargon. No onboarding, no tooltips on key concepts, no "getting started" guide visible in the app.

#### Pain Points
- [ ] No explanation of what "worktrees" are or why they matter
- [ ] No description of what each task (review, implement, design) does
- [ ] No explanation of Auto vs Interactive modes
- [ ] No guidance on what to type in the main input field
- [ ] Random worktree names (NameGenerator.generate()) are confusing

---

### Claude Code Power User

**First impression (0-5 seconds)**:
User sees a clean native macOS app with a familiar sidebar-detail pattern. Recognizes the Task selector with prompt names. Notices token count display.

*Reaction*: "Okay, this is a GUI for `lf` commands. Task selector = prompt files. Token count = context estimation. Makes sense."

**First action**:
User clicks Task dropdown, sees familiar task names (implement, review, design). Selects "implement", types args in the main input, and expects to see the full command being built.

*Issue*: Can't see the actual `lf` command that will be executed. No preview of what will run.

**First obstacle**:
User wants to run with specific flags (`-m codex`, `--voice architect`) but finds:
- Voice selector is hidden under "Options" toggle
- Model selection is not visible in the UI at all
- No way to pass custom flags

*Frustration*: "Where's the model selector? I want to use `-m codex:o3`. The CLI has `--parallel` for model racing—where's that?"

**Recovery**:
User clicks "Options" to reveal Voice selector and Context bar. Finds voices but no model picker. Realizes some features from CLI aren't exposed in GUI.

**Verdict**: Might use occasionally for visual worktree management, but will fall back to CLI for real work. GUI is missing CLI parity on:
- Model selection
- Custom flags
- Parallel model execution
- Pipeline editing (unless beta flag enabled)

#### Pain Points
- [ ] No model selector visible (buried or missing)
- [ ] No way to pass custom CLI flags
- [ ] No command preview showing what will execute
- [ ] No `--parallel` model racing support visible
- [ ] Can't edit pipeline definitions without beta flag
- [ ] No session history view (mentioned in docs but not obvious in UI)

---

### Designer/PM

**First impression (0-5 seconds)**:
User sees WelcomeWindow. Clean design, subtle colors. "Manage worktrees" is confusing—sounds like forestry. Clicks through to SetupView.

*Reaction*: "Install Loopflow? Worktrunk? What are these? I just wanted to try AI coding help."

**First action**:
After setup, user sees the main interface. Types a question like "help me write a product spec for user authentication" in the main input.

*Issue*: Task selector shows technical names (implement, review, polish). No "write docs" or "help me plan" visible.

**First obstacle**:
User types their request and hits Run. A terminal window opens with scrolling text. It's running `claude` commands. User has no idea what's happening or how to interact.

*Confusion points*:
- "Why did a black terminal window open?"
- "What is all this scrolling text?"
- "How do I talk to it? Where do I type?"
- "It says 'Auto' mode—what does that mean?"

**Recovery**:
User might notice "Interactive" mode toggle and try that. But still don't understand what a "worktree" is or why one was created.

**Verdict**: Would not come back. The app assumes deep familiarity with:
- Git worktrees
- Command-line interfaces
- Claude Code / LLM coding assistants

No explanation of these concepts. No friendly onboarding for non-engineers.

#### Pain Points
- [ ] Jargon-heavy: "worktrees", "prompts", "pipelines", "voices"
- [ ] No task descriptions—what does "polish" do vs "review"?
- [ ] Terminal output frightening for non-technical users
- [ ] No visual feedback in the app itself—everything happens in external terminal
- [ ] No templates or examples for non-code tasks (docs, specs, planning)

---

## Top 5 Priority Issues

1. **No onboarding or explanation of core concepts**
   - Worktrees, tasks, modes, and context options are unexplained
   - New users are lost immediately
   - Fix: Add first-run tutorial or tooltips on hover

2. **Missing model selector in UI**
   - CLI's `-m` flag has no GUI equivalent
   - Power users can't select claude:opus vs codex:o3
   - Fix: Add model picker to the options bar

3. **Task selector doesn't explain what tasks do**
   - Dropdown shows names but no descriptions
   - Users can't make informed choices
   - Fix: Add short descriptions in dropdown, or a "?" icon linking to docs

4. **No command preview before execution**
   - Users can't see what will actually run
   - Prevents learning and debugging
   - Fix: Show the `lf` command that will execute

5. **Empty states are unhelpful**
   - "No worktrees" doesn't explain what to do
   - "No voices in .lf/voices/" assumes file system knowledge
   - Fix: Add actionable guidance and links to documentation
