# UX Research

## Screenshots Captured
- `.design/screenshots/maestro-main.png` - Main interface showing screen recording permission dialog overlaying Maestro alongside Cursor IDE

**Note**: The captured screenshot shows a macOS "Screen & System Audio Recording" permission dialog requesting access for "MaestroU0.2026.01.14.08.16.sta-9kc_042" - a cryptic auto-generated identifier. This reveals a significant first-run friction point. The analysis below combines screenshot observation with comprehensive code review of all SwiftUI views.

## Visual Issues

### Layout and Spacing
- [x] **Task selector horizontal layout** - Fixed: Task selector uses typeahead field with inline label
- [x] **Worktree empty state centering** - Fixed: Uses 40/60 optical centering
- [x] **Results panel empty state** - Fixed: Shows "Ready to run" with terminal icon
- [ ] **Context bar horizontal overflow** (`PromptLauncher.swift:contextBar`): Five chips (Docs, Files, Diff, Clipboard, Summaries) shown immediately with no max-width constraint - can extend beyond viewport with many attachments
- [ ] **Result panel header density** (`ResultsPanel.swift:resultHeader`): Five controls compete for attention in one row (status icon, text, duration, stop button, overflow menu)
- [ ] **Options section visual separation** (`PromptLauncher.swift`): Model selector, voice selector, context bar, terminal toggle, and command preview lack clear visual grouping

### Typography Hierarchy
- [x] **Sidebar header weight** - Fixed: "Workspaces" with medium weight title case
- [x] **Placeholder text prominence** - Fixed: "What should the AI build?" at secondary opacity with examples
- [x] **Task dropdown descriptions** - Fixed: 2-line limit with caption font
- [ ] **Mixed caption sizing**: Inconsistent use of `.caption` vs `.caption2` across views - some secondary text is `.caption2`, some is `.caption` with `.tertiary` color
- [ ] **Token count visual weight** (`PromptLauncher.swift`): Small monospaced button blends in with other secondary controls, neither prominent enough to draw attention nor hidden enough to be ignored

### Color and Contrast
- [x] **Stage badge accessibility** - Fixed: Icons added (lightbulb, hammer, magnifyingglass, sparkles) alongside colors
- [x] **Running state accessibility** - Fixed: Shows "Running" text when reduced motion enabled
- [x] **Selected worktree highlight** - Fixed: Increased to 0.25 opacity
- [ ] **Context chip opacity when disabled** (`PromptLauncher.swift`): Disabled sections at 0.5 opacity may fail WCAG AA 4.5:1 contrast
- [ ] **Permission dialog app identifier** (screenshot): System dialog shows gibberish "MaestroU0.2026.01.14.08.16.sta-9kc_042" - confusing and suspicious-looking

### Visual Clutter
- [ ] **Context chips all visible by default** (`PromptLauncher.swift`): Docs, Files, Diff, Clipboard, Summaries shown immediately when expanded - overwhelming for new users
- [ ] **Hover action overload** (`WorktreeSidebar.swift`): Four icons (diff, PR, terminal, IDE) appear on hover with similar size/style - hard to distinguish at a glance
- [ ] **Multiple collapsible sections** (`PromptLauncher.swift`): Context bar, advanced options, and command preview can all be expanded simultaneously, creating visual complexity

### macOS Platform Conventions
- [x] **Repo name duplication** - Fixed: Removed redundant toolbar text item
- [ ] **Permission dialog identity**: Bundle identifier shown in system dialogs is cryptic gibberish
- [ ] **No File > Open Recent**: Recent repos only accessible from welcome window - no way to switch repos while working without returning to welcome
- [ ] **Modal sheets**: Worktree creation and diff views use sheets with fixed sizing that may feel cramped on larger displays

## User Profile Findings

### New Developer

**First impression** (0-5 seconds):
Opens Maestro and sees a clean split-panel interface. The left sidebar shows "Workspaces" with a friendly empty state:

> "No workspaces yet"
> "Create a workspace to let AI work on a feature without affecting your main code."

This is clear and non-intimidating. The icon (stacked boxes) suggests organization.

The main area has a large text input with an inviting placeholder:

> "What should the AI build?"
> "Try: 'add user authentication' or 'refactor the API to use async/await'"

Concrete examples help them understand what to type. Above it, "Task" shows "implement" pre-selected - a sensible default they don't need to understand.

Below the input, they see:
- Mode picker: "Auto" | "Interactive" - meaningless labels
- Token count "2.5k" - what's a token?
- "More options" - suggests complexity is hidden (good)

**First action**:
They type "add a login form to the homepage" in the text area. The pre-selected "implement" task seems reasonable. They spot the Run button with "⌘↵" shortcut - familiar macOS pattern. They click Run.

**First obstacle**:
Two scenarios unfold:

1. **If no worktree selected**: A worktree is auto-created with a generated name like "floral-tiger". This appears in the sidebar but provides no context about what just happened or where the work will happen. The name is whimsical but meaningless.

2. **If "Show output in app" is enabled**: An embedded terminal appears in the results panel showing Claude Code's output. Lines stream by with arrows and technical output. They can watch but don't fully understand what's happening.

3. **If "Show output in app" is disabled**: A terminal window opens separately (Warp or Terminal). Maestro shows "Running implement..." in the results panel but the actual work happens elsewhere. They must context-switch to see progress.

The disconnect between "launch here, watch there" breaks their mental model. If they lose the terminal window, there's no obvious way to recover.

**Recovery**:
When the task completes, the results panel shows "implement completed" with a duration and files changed. The "View Full Diff" button is helpful. But if it ran in external terminal, they may have missed it entirely.

**Verdict**:
**Would cautiously return.** The prompt input is welcoming - concrete examples, clear action. But everything after pressing Run requires developer knowledge. "Workspaces" is friendlier than "branches" but still abstract. The embedded terminal is a significant improvement over external-only output - if they discovered the toggle. Overall, feels like 80% of a great experience with 20% confusion.

#### Pain Points
- [ ] Auto-generated workspace names ("floral-tiger") are cute but don't explain what's happening or where
- [ ] "Auto" vs "Interactive" mode picker has no explanation - tooltips exist but must be discovered
- [ ] Token count assumes LLM knowledge - "2.5k" means nothing to newcomers
- [ ] Context chips (Docs, Files, Diff, etc.) are jargon even when expanded - no "what does this include?"
- [ ] No visual indication of what will happen before pressing Run - the command preview is hidden by default
- [ ] If using external terminal: results appear elsewhere, breaking flow
- [ ] No notification when a background task completes

### Claude Code Power User

**First impression** (0-5 seconds):
Immediately recognizes this as a loopflow GUI. The task selector shows available prompts - "this is `lf <task>`". Token count makes sense. Context chips (Docs, Files, Diff) map directly to CLI flags they know.

Sidebar shows "Workspaces" with familiar worktree information: branch names, commit counts, stage badges (design/implement/review/polish). The icons on stage badges are a nice accessibility touch - can distinguish states even with color blindness.

They note the "More options" toggle reveals model selector, voice selector, context bar - exactly the CLI flags. The command preview (when found) shows the exact `lf` command that will run. This is the transparency they want.

**First action**:
Select a worktree from the sidebar. Type a prompt. Expand "More options" to check model and voice settings. Look for command preview to verify what will run.

**First obstacle**:
Command preview exists but is collapsed by default behind "More options" and then another collapse. They want to:
- See the exact command before running (need to expand twice)
- Add custom flags like `--parallel` (not exposed in UI)
- Use a model not in the dropdown (can't enter arbitrary strings)

The model selector shows common models (`claude:opus`, `claude:sonnet`, `codex:o3`) but power users may need `claude:haiku` or custom configurations. No way to enter arbitrary CLI flags.

The embedded terminal toggle is great - auto mode output streams directly in-app. But interactive mode still launches external terminal (documented constraint).

**Recovery**:
They copy the command from the preview and run it directly in terminal for more control. The app becomes a "visual launcher with worktree management" rather than complete workflow tool.

**Verdict**:
**Would use for specific features.** Main value: visual worktree status, quick launching, diff/compare views, PR management. Missing: arbitrary CLI flags, custom model strings, always-visible command preview. They'd use Maestro alongside CLI - launching simple tasks from GUI, complex ones from terminal.

#### Pain Points
- [ ] Command preview collapsed by default - requires two levels of expansion to see
- [ ] No custom CLI flags (`--parallel`, `--no-diff`, arbitrary options)
- [ ] Model selector shows only preset list - can't enter custom model strings
- [ ] Can't create/edit voice files from the UI (must create .md files manually)
- [ ] No diff preview before running - only see what will be included via context preview
- [ ] Pipelines/Agents behind `Flags.beta` with no discovery path for regular users
- [ ] No keyboard shortcut to expand command preview directly
- [ ] "Show output in app" toggle buried in advanced options - should be more prominent for auto mode

### Designer/PM

**First impression** (0-5 seconds):
Clean macOS app with familiar split-view. The welcome screen says "Tell it what to build. It writes the code." - concrete and promising.

The left sidebar says "Workspaces" with an empty state explaining:

> "Create a workspace to let AI work on a feature without affecting your main code."

Still somewhat technical but the "without affecting your main code" part is reassuring.

The main area has a large text field with "What should the AI build?" - feels like ChatGPT or a search bar. Very inviting.

Below they see:
- "Task" showing "implement" - what's a task?
- "Auto" | "Interactive" toggle - unclear
- Token count "14.2k" - meaningless
- Colored chips: Docs, Files, Diff, Clipboard - developer jargon

**First action**:
Type "write a summary of what this project does" - they want documentation, not code. The task dropdown shows options but:
- "implement: Turn design doc into working code" - not what they want
- "review: Review diff and produce assessment" - too technical
- "design: Produce implementation spec" - closer but still code-focused

There's no obvious "explain" or "document" task. They pick "implement" because it's the default.

**First obstacle**:
Multiple friction points stack up:

1. **Task mismatch**: All available tasks assume code changes. Documentation/explanation isn't a first-class task.

2. **Workspace creation**: When they run, a worktree named "sunny-koala" appears in sidebar. Despite the explanatory text, they don't understand what this represents or where files go.

3. **Terminal output** (if embedded): Lines stream by with arrows (→), file paths, git commands. Claude Code's output is developer-speak even with friendly formatting.

4. **Results summary**: "3 files changed, +45 -12" - completely meaningless. They don't know what a line count represents or why it matters.

5. **Context chips are overwhelming**: If they explored "More options", they'd see Docs, Files, Diff, Clipboard, Summaries toggles. No idea what these include or why they matter.

**Recovery**:
They'd need someone to explain:
- Tasks are for code changes, not general queries
- The terminal is where the AI "thinks out loud"
- A workspace is an isolated folder for the AI's changes
- Line counts show what changed

The app provides none of this scaffolding in-context. Tooltips help but must be discovered.

**Verdict**:
**Would not return without guidance.** The prompt input is welcoming, but everything after pressing Run assumes developer mental models. Git concepts, terminal output, diff statistics - all foreign. A PM writing specs or designer prototyping UI would feel lost. They might succeed with hand-holding but wouldn't independently adopt the tool.

#### Pain Points
- [ ] No task for "explain" or "document" - all tasks assume code modification
- [ ] Task descriptions use technical language ("diff", "design doc", "spec")
- [ ] Terminal output is intimidating and unexplained - even embedded output is technical
- [ ] Git concepts (workspace, branch, diff, commit) assumed known
- [ ] No "simple mode" that hides technical complexity
- [ ] Results panel speaks developer language (line counts, file diffs)
- [ ] Token count displayed but never explained anywhere
- [ ] No onboarding flow explaining the workflow
- [ ] Colored chips are pretty but meaningless without context
- [ ] No link to documentation or help from the interface

## Top 5 Priority Issues

### 1. Embedded terminal toggle buried in advanced options
**Impact**: Critical for all profiles
**Location**: `PromptLauncher.swift` → embeddedTerminalToggle in showAdvancedOptions section
**Issue**: The "Show output in app" toggle that enables in-app output streaming is hidden behind "More options". New users may never find it, experiencing the jarring context-switch to external terminal. The embedded terminal (via SwiftTerm) is implemented and works well, but discovery is poor.
**Fix**: Make embedded terminal the default for auto mode. Move toggle to more prominent location or remove entirely (always embed for auto, external for interactive as currently designed).

### 2. No onboarding flow for first-time users
**Impact**: High for all profiles
**Location**: App entry point - no walkthrough exists
**Issue**: Full interface shown immediately with no guided introduction. Users must discover tooltips, understand terminology, and figure out workflow independently.
**Fix**: Add 3-4 step walkthrough covering: (1) What Maestro does, (2) How workspaces isolate work, (3) Running your first task, (4) Where to see results. Use the SetupView progress indicator pattern as a model.

### 3. "Auto" vs "Interactive" mode picker is meaningless
**Impact**: High for New Developer, Designer/PM; Medium for Power User
**Location**: `PromptLauncher.swift` mode picker (segmented control)
**Issue**: Two-option segmented control with no explanation of what each mode does. Tooltip exists via `.help()` but requires hover to discover.
**Fix Options**: (a) Add inline description below picker, (b) Rename to "Run to completion" vs "Chat mode", (c) Remove picker entirely - let task frontmatter control mode, default to auto.

### 4. Context controls visible but unexplained
**Impact**: High for New Developer, Designer/PM
**Location**: `PromptLauncher.swift` contextBar section (when expanded)
**Issue**: When context bar is expanded, five chips (Docs, Files, Diff, Clipboard, Summaries) appear with no explanation. Token count unexplained. Users make decisions about things they don't understand.
**Fix**: Keep context bar collapsed by default (already implemented via `@AppStorage`). Add brief descriptions on hover or expand. Add "?" help icon linking to documentation.

### 5. Command preview hidden when power users need transparency
**Impact**: High for Power User
**Location**: `PromptLauncher.swift` commandPreview section
**Issue**: Power users want to see exactly what will run. Currently requires: expand "More options" → scroll down → expand "Command Preview". Three steps to reach critical information.
**Fix**: Remember expansion state via `@AppStorage` (partially implemented). Add keyboard shortcut (Cmd+Shift+P?) to toggle preview visibility directly. Consider showing preview by default for users who have expanded it before.

## Additional Observations

### Improvements Since Last Research (Already Applied)
- [x] **"BRANCHES" → "Workspaces"**: Friendlier terminology with helpful description
- [x] **Stage badges with icons**: Lightbulb/hammer/magnifyingglass/sparkles for accessibility
- [x] **Task pre-selected to "implement"**: Smart default saves decision
- [x] **Better placeholder text**: "What should the AI build?" with concrete examples
- [x] **2-line task descriptions**: More context visible in dropdown
- [x] **"More options" label**: Better than generic "Options" when collapsed
- [x] **Welcome tagline**: "Tell it what to build. It writes the code." - concrete
- [x] **Removed redundant repo name**: Was in both title and toolbar
- [x] **Optical centering for empty state**: Content appears above center for better visual balance
- [x] **Running state accessibility**: Static "Running" text for reduced motion
- [x] **Results panel empty state**: "Ready to run" with explanatory text
- [x] **Embedded terminal via SwiftTerm**: Output can stream in-app (when toggle enabled)
- [x] **Context bar collapsible**: Collapsed by default via `@AppStorage("contextBarExpanded")`

### Positive Patterns Worth Preserving
- **SetupView** has clear 3-step progress indicator with helpful descriptions - good model for onboarding
- **WelcomeWindow** recent repos list is fast and useful for returning users
- **Task dropdown** shows mode badge (auto/interactive) - good information density
- **Keyboard shortcuts** follow standard macOS patterns (Cmd+Enter to run, Cmd+L to focus prompt, Cmd+N new workspace)
- **DiffSheet/CompareSheet** excellent diff visualization with syntax highlighting
- **ResultsPanel** file changes are expandable with diff preview - good progressive disclosure
- **Context preview panel** shows exactly what's being sent - great transparency for those who find it
- **Drag-and-drop** file attachment with visual feedback (blue border)
- **Tooltips** present on most interactive elements via `.help()` modifiers
- **EmbeddedTerminalView** handles PTY and VT100 rendering cleanly

### Missing Affordances
- No "?" help buttons or contextual help links in main interface
- No link to documentation or getting-started guide
- No undo/cancel for running tasks (stop button exists but no undo)
- No notification when background task completes
- No Cmd+K command palette (mentioned in DESIGN.md but not implemented)
- No way to see what files will be included before running without expanding context preview
- No visual connection between workspace selection and where output goes

### Technical Debt Affecting UX
- **Flags.beta** hides Pipelines/Agents with no UI discovery path
- **NameGenerator** whimsical names - delightful for power users, confusing for newcomers
- **Session tracking requires lfd daemon** - if not running, features silently degrade
- **Screen capture permission** shows cryptic bundle identifier in system dialog
- **Interactive mode** still requires external terminal - embedded terminal only for auto mode (documented constraint)

### Design Principle Alignment
Comparing against `Maestro/DESIGN.md`:

| Principle | Current State | Status |
|-----------|---------------|--------|
| Immediate Connection (Bret Victor) | Embedded terminal streams output when enabled | Good (if discovered) |
| Progressive Disclosure (Notion) | "More options" helps, context bar collapses | Good |
| Speed as Feature (Linear) | UI responsive, launches quickly | Good |
| Keyboard-First (Linear) | Cmd+Enter, Cmd+L, Cmd+N work | Partial - no Cmd+K palette |
| Opinionated Defaults (Linear) | Task pre-selected, embedded terminal available | Good |
| Transparency (Cursor) | Command preview available but buried | Partial |
| Design Should Disappear (Ive) | Clean layout, collapsed sections reduce noise | Good |
| Remove Barriers (fast.ai) | Requires git/loopflow understanding | Gap |
| Craft Signals Care (Ive) | Polish present, consistent styling | Good |

## Open Questions

Captured in `.design/questions.md`:

1. **Embedded terminal default**: Should "Show output in app" be enabled by default for auto mode? Current: off by default, toggle buried in advanced options.

2. **Onboarding flow scope**: What should the walkthrough cover? Candidates: (1) What Maestro does, (2) How workspaces isolate work, (3) Running your first task, (4) Where results appear. Should it be skippable?

3. **Mode picker visibility**: Should the Auto/Interactive picker be visible at all? Most users don't need to change it. Could be: (a) hidden entirely, (b) shown only when task frontmatter differs, (c) explained inline.

4. **Non-developer tasks**: Should Maestro support "explain" or "summarize" tasks for non-code users? Or is this out of scope?

5. **Workspace naming**: Keep whimsical names ("floral-tiger") for power users? Or use task-based names ("implement-auth")? Or let users name workspaces?

6. **Command preview prominence**: Show by default for power users (via @AppStorage)? Add keyboard shortcut? Move above the fold?

7. **Help integration**: Add "?" buttons linking to docs? In-app help panel? Contextual tooltips with "Learn more" links?

8. **Permission dialog identity**: Can we control the bundle identifier shown in system dialogs? Current gibberish erodes trust.
