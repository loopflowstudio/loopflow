# UX Research

## Screenshots Captured
- `.design/screenshots/maestro-main.png` - Main interface with repo open (shows modal dialog overlay for screen recording permission)

**Note**: Screenshot capture was limited due to system permissions dialog blocking the view. Analysis based on captured screenshot combined with comprehensive code review of all SwiftUI views.

## Visual Issues

### Layout and Spacing
- [ ] **Task selector label misalignment** (`PromptLauncher.swift:113-117`): "Task" label uses fixed spacing that doesn't align visually with the input field below
- [ ] **Context bar chips crowding** (`PromptLauncher.swift:944-1007`): No maximum width constraint - chips overflow horizontally with many attached files
- [ ] **Worktree empty state vertical centering** (`WorktreeSidebar.swift:168-196`): Uses `maxHeight: .infinity` which can push content too far down in tall windows
- [ ] **Result panel header density** (`ResultsPanel.swift:64-120`): Too many small icons crammed into a single row - status, duration, toggle, clear, expand all compete for attention

### Typography Hierarchy
- [ ] **Inconsistent caption sizing**: Mix of `.caption`, `.caption2`, and custom sizes throughout - no clear hierarchy (e.g., `WorktreeRow` uses `.caption2` for task badges but `.caption` for commit text)
- [ ] **"BRANCHES" header weight** (`WorktreeSidebar.swift:147-149`): All-caps with `.semibold` is visually aggressive for a sidebar section header
- [ ] **Placeholder text hierarchy** (`PromptLauncher.swift:564-572`): Primary placeholder "Describe what you want to build..." uses `.tertiary`, example text uses `.quaternary` - the hierarchy is correct but the primary text is too faded

### Color and Contrast
- [ ] **Disabled state differentiation** (`PromptLauncher.swift:886`): Disabled sections use `opacity(0.5)` which may not meet WCAG contrast requirements
- [ ] **Running state indicator** (`WorktreeRow.swift:659-664`): Pulsing blue dot animation may be missed by users with motion sensitivity
- [ ] **Stage badge colors** (`WorktreeRow.swift:688-696`): Color-only differentiation for task stages (design=blue, implement=purple, review=orange, polish=green) - no shape or icon variation for color-blind users

### Visual Clutter
- [ ] **Too many context chips visible by default** (`PromptLauncher.swift:944-959`): Docs, Files, Diff, Clipboard, Summaries all shown - overwhelming for new users
- [ ] **Hover action button overload** (`WorktreeRow.swift:593-647`): Four action buttons appear on hover - diff, PR, terminal, IDE - all with similar small icons

### macOS Platform Conventions
- [ ] **Non-standard toolbar** (`ContentView.swift:67-84`): Shows repo name twice - in title bar and as a toolbar text item
- [ ] **Sheet sizing** (`NewWorktreeSheet.swift:756`): Fixed 320pt width may feel cramped on larger displays
- [ ] **Window title redundancy** (`ContentView.swift:66`): Shows repo name in both `.navigationTitle()` and toolbar

## User Profile Findings

### New Developer

**First impression** (0-5 seconds):
User sees a split-panel interface with "BRANCHES" in the sidebar and a large text area in the main panel saying "Describe what you want to build..." They notice the "Task" dropdown at top but it says "Select task..." with no obvious indication of what tasks are available. The app looks clean but sparse - not immediately clear what this does beyond being some kind of code-related tool.

**First action**:
They click the "Task" dropdown to see what's available. A list appears showing items like "design", "implement", "review" with cryptic mode labels like "auto" and "interactive". They don't understand what these terms mean. They might try typing something like "add a button" in the main text area.

**First obstacle**:
They type "add a login button" in the text area and hit Run. The system either:
1. Creates a worktree with an auto-generated name like "floral-tiger" (confusing - where did that come from?)
2. Shows an error if no worktree is selected

They don't understand what a "worktree" is. The sidebar says "No worktrees yet" with explanation "Each worktree is an isolated folder where AI can work without affecting your main code" - this is helpful text but "worktree" is git jargon they may not know.

**Recovery**:
If they click "Create Worktree", they see a form asking for "Branch name" and "Base branch". These are git concepts they understand somewhat. They might create one called "login-feature". After creation, they see it in the sidebar but the connection between the worktree and their prompt isn't clear - they have to re-enter their prompt.

**Verdict**:
They'd probably struggle for 5-10 minutes, maybe get something to run once, then be unsure what actually happened. The app doesn't show them the result directly - it launches in a terminal. If they don't know how to check the terminal, they're stuck. **Unlikely to return tomorrow** without guidance.

#### Pain Points
- [ ] No onboarding or tutorial for core concepts (worktrees, tasks, auto vs interactive)
- [ ] No explanation of what happens when you hit "Run"
- [ ] Results appear in external terminal, not in app
- [ ] Auto-generated worktree names ("floral-tiger") are whimsical but confusing
- [ ] "BRANCHES" header doesn't explain that this is where AI work happens
- [ ] Task descriptions (when visible) are too brief - "auto" vs "interactive" meaningless to newcomers

### Claude Code Power User

**First impression** (0-5 seconds):
User recognizes this as a GUI wrapper for loopflow/claude. They immediately see the task selector and think "okay, this is like running `lf <task>`". They notice the token count in the corner and the context chips (Docs, Files, Diff, Clipboard) - familiar concepts. The sidebar shows worktrees which makes sense. They appreciate that it shows the mode (auto/interactive) for each task.

**First action**:
They select "implement" from the task dropdown, type their feature request, and look for the keyboard shortcut. They find Cmd+Enter which is expected. They check the "Options" section to see what else is available - model selector, voice selector, context toggles.

**First obstacle**:
They want to see the exact command that will run (they're used to `lf implement: ...` syntax). They find the "Command Preview" section but it's collapsed by default and nested under Options. They also notice there's no way to run with `--parallel` or other advanced CLI flags directly.

**Recovery**:
They use the app successfully but find themselves wanting to copy the command and paste it into terminal for more control. The app works but feels like it's hiding capabilities they know exist. They might toggle the beta flag to see Pipelines and Agents sections.

**Verdict**:
The app is useful for quick launches and getting a visual overview of worktrees. They'd use it as a "dashboard" alongside terminal work, but **would continue using CLI for serious work**. Main value: seeing worktree status at a glance, quick PR creation, launching tasks without typing paths.

#### Pain Points
- [ ] Command preview is hidden by default - power users want to see what's actually running
- [ ] No way to add arbitrary CLI flags (`--parallel`, `--no-diff`, etc.)
- [ ] Results panel shows summary but real action is in terminal
- [ ] Can't see live streaming output - have to switch to terminal
- [ ] Model selector doesn't show all available options (just "common models")
- [ ] Voice selector is basic - can't quickly edit voice files
- [ ] No diff preview before running - have to open terminal to see current state

### Designer/PM

**First impression** (0-5 seconds):
User sees a clean macOS app with a familiar split-view layout. The header "Loopflow Maestro" with "AI coding assistant" description sets expectations. They see the big text area with "Describe what you want to build..." which is inviting. However, the sidebar showing "BRANCHES" with git-like terminology is immediately intimidating.

**First action**:
They try the most obvious thing - type something in the text box. They write "add a new page for user settings" and look for how to proceed. They see the Run button with Cmd+Enter shortcut. They hesitate because they're not sure what "Task" should be selected - is "implement" right? What does "auto" mean?

**First obstacle**:
Multiple friction points compound:
1. "Task" selector defaults to empty ("Select task...") - which one to pick?
2. "Auto" vs "Interactive" toggle has hover tooltips but tooltips require hovering
3. When they run, it wants to create a worktree - what's that?
4. The action launches a terminal window - they're not comfortable with terminals
5. They can't see progress in the app - just a small "Running..." indicator

**Recovery**:
They would need someone to explain the workflow. The app doesn't provide enough scaffolding for non-technical users. They might close the terminal window accidentally and lose track of what's happening.

**Verdict**:
**Would not return**. The app assumes too much technical knowledge. Key concepts (worktrees, tasks, auto/interactive, terminal) are presented without sufficient explanation. A PM trying to write a spec or a designer trying to prototype CSS changes would feel lost.

#### Pain Points
- [ ] No progressive disclosure - advanced features visible immediately
- [ ] Task selection required without explaining what tasks do
- [ ] Results go to terminal instead of showing in-app
- [ ] Git terminology ("worktree", "branch", "diff") used without explanation
- [ ] No "simple mode" for non-engineers
- [ ] Token count display assumes user knows what tokens mean
- [ ] Context options (Docs, Files, Diff) are developer-centric labels
- [ ] No example prompts or templates to start from

## Top 5 Priority Issues

1. **No onboarding flow for new users**
   - First-time users see the full interface without any explanation of core concepts
   - Fix: Add a 3-step walkthrough explaining worktrees, tasks, and how to see results
   - Impact: All user profiles struggle on first use

2. **Results appear in external terminal, not in-app**
   - Users can't see what the AI is doing without switching applications
   - Fix: Add in-app streaming output panel or at minimum clear status with progress
   - Impact: New Developer and Designer/PM lose context; Power User tolerates it

3. **Task selector doesn't explain what tasks do**
   - "design", "implement", "review" are verbs with no description of their purpose
   - Fix: Add descriptions to task dropdown (visible, not just tooltip); show task prompt preview
   - Impact: New Developer guesses; Designer/PM confused

4. **Worktree concept introduced without explanation**
   - "Each worktree is an isolated folder" is shown only in empty state
   - Fix: Add persistent help text or "?" icon that explains why worktrees matter
   - Impact: New Developer and Designer/PM don't understand the core model

5. **Too many options visible by default for non-power-users**
   - Context chips (Docs, Files, Diff, Clipboard, Summaries), model selector, voice selector all visible
   - Fix: Hide advanced options behind "Advanced" toggle; show sensible defaults
   - Impact: Designer/PM overwhelmed; New Developer confused by choices

## Additional Observations from Code Review

### Positive Patterns
- **SetupView** (`SetupView.swift:1-285`) provides a decent first-run experience for installing dependencies
- **Welcome screen** (`WelcomeWindow.swift`) with recent repos is helpful for returning users
- **Task dropdown** (`PromptLauncher.swift:192-226`) now shows descriptions when available
- **Keyboard shortcuts** are standard (Cmd+Enter to run, Cmd+L to focus prompt)
- **Diff viewer** (`WorktreeSidebar.swift:776-883`) is well-designed with syntax highlighting

### Missing Affordances
- No "?" help buttons anywhere in the UI
- No links to documentation
- No tooltips on most interactive elements
- No empty state guidance beyond the worktree sidebar
- No undo/cancel for running tasks
- No notification when background task completes (user must watch terminal)

### Technical Debt Affecting UX
- Beta flag (`Flags.beta`) hides Pipelines and Agents - unclear how users discover these
- `showAdvancedOptions` state means settings are hidden by default
- `selectedPipeline` state switching changes entire main view - potentially disorienting
