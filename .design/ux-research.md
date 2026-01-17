# UX Research

## Screenshots Captured
- `.design/screenshots/maestro-main.png` - Main interface with repo open (shows modal dialog overlay for screen recording permission)

**Note**: Screenshot capture was limited due to system permissions dialog blocking the view. Analysis based on captured screenshot combined with comprehensive code review of all SwiftUI views.

## Visual Issues

### Layout and Spacing
- [ ] **Task selector horizontal layout** (`PromptLauncher.swift:113-117`): "Task" label at `.caption` sits beside the typeahead field; visual hierarchy suggests it should be above, not inline
- [ ] **Context bar horizontal overflow** (`PromptLauncher.swift:944-1007`): No maximum width or wrapping - chips extend beyond visible area with many attached files
- [ ] **Worktree empty state centering** (`WorktreeSidebar.swift:168-196`): Uses `.frame(maxWidth: .infinity, maxHeight: .infinity)` pushing content low in tall windows
- [ ] **Result panel header density** (`ResultsPanel.swift:64-120`): Five controls crammed into one row (status, text, duration, toggle, clear, expand) - competes for attention
- [ ] **Options section lacks grouping** (`PromptLauncher.swift:60-66`): Model selector, voice selector, context bar, and command preview run together without visual separation

### Typography Hierarchy
- [ ] **Inconsistent caption sizing**: Mix of `.caption`, `.caption2`, custom sizes without clear system (e.g., `WorktreeRow` uses `.caption2` for badges but `.caption` for commit text)
- [ ] **"BRANCHES" header** (`WorktreeSidebar.swift:147-149`): All-caps `.semibold` is visually heavy for sidebar chrome
- [ ] **Placeholder text fading** (`PromptLauncher.swift:564-572`): Primary placeholder at `.tertiary` is too faded - first thing users see should be more prominent
- [ ] **Task dropdown descriptions** (`PromptLauncher.swift:213-218`): `.caption2` descriptions truncated to 1 line - valuable context gets cut off

### Color and Contrast
- [ ] **Disabled state opacity** (`PromptLauncher.swift:886`): Context sections at `opacity(0.5)` may not meet WCAG 4.5:1 contrast
- [ ] **Running state indicator** (`WorktreeRow.swift:659-664`): Pulsing blue dot may be missed by users with motion sensitivity
- [ ] **Stage badge colors** (`WorktreeRow.swift:688-696`): Color-only differentiation (design=blue, implement=purple, review=orange, polish=green) - no icon for color-blind users
- [ ] **Selected worktree** (`WorktreeRow.swift:527-530`): Blue accent at 15% opacity may be too subtle

### Visual Clutter
- [ ] **Context chips all visible** (`PromptLauncher.swift:947-958`): Docs, Files, Diff, Clipboard, Summaries shown immediately - overwhelming for new users
- [ ] **Hover action overload** (`WorktreeRow.swift:593-647`): Four icons (diff, PR, terminal, IDE) on hover - all similar size/style, hard to distinguish
- [ ] **Token count placement** (`PromptLauncher.swift:740-756`): Embedded in crowded row with mode picker and options button

### macOS Platform Conventions
- [ ] **Repo name duplication** (`ContentView.swift:66-83`): Appears in `.navigationTitle()` AND as toolbar text item
- [ ] **Sheet sizing** (`NewWorktreeSheet.swift:756`): Fixed 320pt width feels cramped on larger displays
- [ ] **No File > Open Recent**: Recent repos only in welcome window, not accessible while working

## User Profile Findings

### New Developer

**First impression** (0-5 seconds):
User sees a split-panel interface with "BRANCHES" in the sidebar and a large text area. The placeholder "Describe what you want to build..." is promising - sounds like ChatGPT for code. They notice "Task" dropdown saying "Select task..." but have no idea what tasks are available or what they do.

The sidebar shows "No worktrees yet" with explanation: "Each worktree is an isolated folder where AI can work without affecting your main code." Helpful text, but "worktree" is git jargon they don't know.

**First action**:
They click the Task dropdown. A list appears:

```
Tasks
├── design           auto
├── implement        auto
├── review           auto
├── polish           auto
```

Labels "auto" vs "interactive" mean nothing. They might pick "implement" because it sounds like building, or skip the dropdown entirely and type in the text box.

**First obstacle**:
They type "add a login button to the homepage" and press Cmd+Enter. Two scenarios:

1. **With worktree selected**: Terminal window opens with cryptic output. They lose sight of Maestro - action moved to different app.

2. **Without worktree**: System creates one named "floral-tiger" (from `NameGenerator.generate()`). They see this in the sidebar but have no idea where that name came from.

They don't understand why results appeared in Terminal instead of in-app. ResultsPanel shows "Running implement..." but actual work happens in external window they may have lost.

**Recovery**:
If they find Terminal, they see Claude Code output scrolling. If they lose it, they're stuck wondering what happened. After completion, "View Full Diff" button in results panel is useful but requires knowing to look there.

**Verdict**:
**Unlikely to return without guidance.** Too many concepts introduced at once (worktrees, tasks, auto/interactive, terminal) without explanation. App assumes loopflow/git familiarity newcomers lack.

#### Pain Points
- [ ] No onboarding explaining worktrees, tasks, or modes
- [ ] No indication of what happens when you press Run
- [ ] Results in external terminal, not in-app
- [ ] Auto-generated worktree names confusing ("floral-tiger"?)
- [ ] Task descriptions truncated - can't understand purpose
- [ ] "auto" vs "interactive" meaningless without context
- [ ] No examples showing what's possible

### Claude Code Power User

**First impression** (0-5 seconds):
Immediately recognizes this as loopflow GUI. Task dropdown is "oh, this is `lf <task>`". Token count makes sense - they know context windows. Context chips map to CLI flags. Sidebar worktrees exactly what they expect. Appreciate task badges and running state indicators.

**First action**:
Select "implement", type feature, check Options: model selector, voice selector, context toggles - equivalent to CLI flags. Look for command preview to verify what runs.

**First obstacle**:
Command preview exists (`PromptLauncher.swift:1050-1095`) but collapsed by default under "Command Preview" header. Want to add `--parallel` but no UI for arbitrary CLI flags. Model selector shows only common models, not all options.

**Recovery**:
Use app successfully but find themselves copying command preview to terminal for more control. App becomes "launcher" rather than complete workflow. Diff viewer and comparison features are valuable.

**Verdict**:
**Would use as dashboard, continue CLI for serious work.** Main value: visual worktree status, quick launching, PR management. Missing: live output, advanced flags, full model list.

#### Pain Points
- [ ] Command preview collapsed by default
- [ ] No custom CLI flags (--parallel, --no-diff, etc.)
- [ ] Live output streams to terminal, not in-app
- [ ] Model selector shows only "common models"
- [ ] Can't create/edit voice files inline
- [ ] No diff preview before running
- [ ] Pipelines/Agents behind `Flags.beta` with no discovery path

### Designer/PM

**First impression** (0-5 seconds):
Clean macOS app, familiar split-view. Big text area "Describe what you want to build..." is inviting - plain language input. But below:
- "Task" dropdown with technical options
- "Auto" vs "Interactive" with no explanation
- Colorful chips (Docs, Files, Diff, Clipboard)
- Token count "14.2k" - what does that mean?

Sidebar "BRANCHES" suggests git - "something developers use." Text about "affecting your main code" makes them wonder if their work is at risk.

**First action**:
Type "write a summary of what this codebase does" (trying for documentation, not coding). Not sure what Task to select. Pick "design" because sounds less technical. Press Run.

**First obstacle**:
Multiple friction points:
1. **Worktree creation**: Prompted to create "sunny-koala" - no idea what/why
2. **Terminal launch**: Opens Terminal - foreign territory
3. **No visible progress**: Maestro shows "Running design..." but nothing happening there
4. **Technical output**: Terminal shows file paths, git commands - not addressing their question
5. **Results confusion**: "files changed" and "commits" - developer concepts they don't need

**Recovery**:
Need someone to explain:
- What a worktree is and why
- That terminal shows progress, not Maestro
- That "design" is for software architecture, not documentation

App doesn't provide this scaffolding.

**Verdict**:
**Would not return.** App assumes developer mental models. Git terminology, terminal output, no simple mode or guided experience. PM writing specs or designer prototyping UI would be lost within first minute.

#### Pain Points
- [ ] All advanced options visible immediately
- [ ] Task selector requires understanding dev phases
- [ ] Results in external terminal - non-technical users avoid terminals
- [ ] Git jargon: "worktree", "branch", "diff", "commit"
- [ ] No "simple mode" hiding complexity
- [ ] Token count assumes LLM knowledge
- [ ] Context chips are developer-centric labels
- [ ] No example prompts
- [ ] Technical error messages: "Failed to create worktree"

## Top 5 Priority Issues

1. **No onboarding flow for first-time users**
   - First-time users see full interface without explanation of core concepts
   - Location: App entry point, needs new `OnboardingView.swift`
   - Impact: All profiles struggle on first launch
   - Fix: 3-4 step walkthrough covering worktrees, tasks, where to see results

2. **Results appear in external terminal, not in-app**
   - Users can't see AI activity without switching applications
   - Location: `PromptLauncher.swift:1191-1200` launches external terminal
   - Impact: New Developer and Designer/PM lose context; Power User tolerates it
   - Fix: Embedded PTY or websocket output panel (high value, technical challenge)

3. **Task selector doesn't explain what tasks do**
   - "design", "implement", "review" are verbs without description of purpose
   - Location: `PromptLauncher.swift:200-226` shows description but truncated
   - Impact: New Developer guesses; Designer/PM chooses randomly
   - Fix: Full task description in dropdown or hover cards explaining each task

4. **Worktree concept introduced without explanation**
   - "Isolated folder" explanation only in empty state
   - Location: `WorktreeSidebar.swift:168-196`
   - Impact: New Developer and Designer/PM don't understand core model
   - Fix: Persistent help tooltip on "BRANCHES" header; friendlier term ("workspace"?)

5. **Too many options visible by default**
   - Context chips, model selector, voice selector shown immediately
   - Location: `PromptLauncher.swift:60-66` Options section
   - Impact: Designer/PM overwhelmed; New Developer confused
   - Fix: Collapse advanced options by default; surface only Run and basic input for new users

## Additional Observations from Code Review

### Positive Patterns
- **SetupView** has clear progress indicators and helpful descriptions - pattern should extend to main app
- **WelcomeWindow** recent repos list useful for returning users
- **Task dropdown** shows descriptions when available (good start, needs more space)
- **Keyboard shortcuts** standard macOS (Cmd+Enter, Cmd+L, Cmd+N)
- **DiffSheet/CompareSheet** excellent diff visualization with syntax highlighting
- **ResultsPanel** file changes with expandable previews - good progressive disclosure

### Missing Affordances
- No "?" help buttons or contextual help anywhere
- No links to documentation or getting-started guide
- Tooltips inconsistent (some `.help()` modifiers, many missing)
- No empty state guidance beyond worktree sidebar
- No undo/cancel for running tasks (terminal can close but no graceful stop)
- No notification when background task completes (requires watching terminal)
- No Cmd+K command palette (despite being in DESIGN.md principles)

### Technical Debt Affecting UX
- **Flags.beta** hides Pipelines/Agents with no UI discovery path
- **showAdvancedOptions** defaults false - collapsed section lacks indicator of what's hidden
- **selectedPipeline** state switch changes entire main view - disorienting
- **NameGenerator** whimsical names - delightful for power users, confusing for newcomers
- **TerminalLauncher** only output path - no in-app fallback

### Design Principle Violations
Comparing against `Maestro/DESIGN.md`:

| Principle | Current State | Gap |
|-----------|---------------|-----|
| Immediate Connection (Bret Victor) | Output streams to external terminal | Users don't see agent progress in real-time within Maestro |
| Progressive Disclosure (Notion) | All options visible or collapsed without preview | No graceful reveal of complexity |
| Speed as Feature (Linear) | Launches quickly, UI responsive | Terminal launch adds context-switch latency |
| Keyboard-First (Linear) | Some shortcuts, no Cmd+K palette | Missing global command search |
| Opinionated Defaults (Linear) | Good defaults but no explanation | Users don't know what defaults mean |
| Transparency (Cursor) | Command preview available but hidden | Should show what will run prominently |
| Design Should Disappear (Ive) | Chrome present but minimal | Sidebar headers feel like "design" |
| Remove Barriers (fast.ai) | Requires git/loopflow understanding | Significant barriers for non-developers |

## Questions for Product

Captured in `.design/questions.md`:

1. **Target audience priority**: Power users wanting dashboard, or accessible to non-technical users?
2. **Results in terminal vs in-app**: Technical feasibility of embedded PTY or websocket streaming?
3. **Beta flag discoverability**: How should users discover Pipelines/Agents features?
4. **Screenshot capture permissions**: Is system dialog the intended UX, or should there be fallback?
5. **Worktree terminology**: Would "workspace", "branch folder", or "isolated copy" be clearer?
