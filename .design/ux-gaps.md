# UX Gap Analysis

## Visual Issues

From the screenshot and code review, the following craft issues need attention:

### Alignment and Spacing
- [ ] **Task selector label misalignment** (`PromptLauncher.swift:113-117`): "Task" label floats left with fixed spacing, doesn't create visual relationship with input field
- [ ] **Context bar chip overflow** (`PromptLauncher.swift:944-1007`): No max-width or wrap—chips run off screen with many attachments
- [ ] **Empty state vertical centering** (`WorktreeSidebar.swift:168-196`): Uses unbounded `maxHeight: .infinity`, content drifts down in tall windows
- [ ] **Result panel header density** (`ResultsPanel.swift:64-120`): Five small icons compete for attention in a single row

### Typography Hierarchy
- [ ] **Inconsistent caption sizing**: Mix of `.caption`, `.caption2`, and custom font sizes with no clear system
- [ ] **"BRANCHES" header** (`WorktreeSidebar.swift:147-149`): All-caps + `.semibold` feels aggressive for navigation
- [ ] **Placeholder text too faded** (`PromptLauncher.swift:564-572`): Primary placeholder uses `.tertiary`—should be more prominent as the main CTA

### Color and Contrast
- [ ] **Disabled state opacity** (`PromptLauncher.swift:886`): `opacity(0.5)` may fail WCAG AA contrast
- [ ] **Running state indicator** (`WorktreeRow.swift:659-664`): Pulsing blue dot relies on animation—accessibility issue for motion-sensitive users
- [ ] **Stage badge colors** (`WorktreeRow.swift:688-696`): Color-only differentiation (design=blue, implement=purple, review=orange, polish=green) fails for color-blind users—no icon or shape variation

### Visual Clutter
- [ ] **Five context chips visible by default** (`PromptLauncher.swift:944-959`): Docs, Files, Diff, Clipboard, Summaries all shown—overwhelming first impression
- [ ] **Four hover action buttons** (`WorktreeRow.swift:593-647`): Diff, PR, Terminal, IDE all appear at once with similar small icons

### macOS Platform Conventions
- [ ] **Redundant repo name display** (`ContentView.swift:66-84`): Shows in both `.navigationTitle()` and toolbar—pick one
- [ ] **Fixed sheet width** (`NewWorktreeSheet.swift:756`): 320pt is cramped on larger displays; should scale

---

## Welcome/Setup

**Current**: `WelcomeWindow.swift` shows a centered icon, title "Loopflow Maestro", subtitle "AI coding assistant for your projects", recent repos list, and "Open Folder" button. `SetupView.swift` handles first-run dependency installation with a 3-step progress indicator.

**Inspiration—Figma**: Figma's onboarding creates value in under 10 seconds. You're drawing immediately. There's no "here's what this is" preamble—you learn by doing. The interface teaches through interaction, not explanation.

**Inspiration—Notion**: Notion's empty state offers templates that demonstrate value. "Press / for commands" teaches the core interaction. You're productive before you understand the tool.

**Gap**: Maestro's welcome screen is a repo picker, not an introduction to the product's value. New users see "AI coding assistant" but don't know what that means or why they should care. The setup flow handles dependencies but doesn't explain the workflow.

**Pattern to adopt**:
- Show a 30-second "here's what happens" micro-tutorial on first launch
- Replace generic subtitle with a concrete example: "Tell it what to build. It writes the code in an isolated branch while you keep working."
- Offer a "Try with a demo project" option that doesn't require picking a repo

---

## Prompt Input

**Current**: `PromptLauncher.swift` shows a task dropdown, large text area with placeholder "Describe what you want to build...", mode picker (Auto/Interactive), and a Run button. Context chips appear below with token count. Command preview is hidden behind "Options" toggle.

**Inspiration—Cursor**: Cursor's chat is immediate. You type, you get a response. Context is automatic. The prompt area adapts to what you're doing—inline for small edits, full panel for conversations. `@` mentions let you surgically add context without mode switches.

**Inspiration—Notion**: Notion's `/` commands are discoverable because they appear as you type. You don't need to know they exist—the system teaches you. The interaction feels like writing, not operating software.

**Gap**: Maestro requires explicit task selection before you can type. The dropdown says "Select task..." with no indication of what tasks do. The text area looks like a search box, not a conversation. Users must click "Options" to see what context they're sending.

**Patterns to adopt**:
1. **Default task**: Pre-select "implement" or infer from prompt content—don't make users choose first
2. **Inline task completion**: As you type "review", show matching tasks inline (like Cursor's @-mentions)
3. **Context preview by default**: Show what's being sent without clicking "Options"—transparency over hidden state
4. **Slash commands**: `/design`, `/review` as alternative to dropdown—typing is faster than clicking
5. **Rich placeholder**: Instead of generic text, show an actual example prompt: `"Add a login page with email/password"`

---

## Context Controls

**Current**: Five toggle chips (Docs, Files, Diff, Clipboard, Summaries) plus drag-and-drop file attachment. Token count expands to show breakdown. `@` mentions don't exist.

**Inspiration—Cursor**: Context is automatic and smart. Cursor reads your cursor position, open files, and recent edits to know what's relevant. You don't toggle "include files"—it just knows. Override is surgical: `@file.ts` to add, not checkboxes.

**Inspiration—Figma**: Figma's component panel shows what you have selected and its properties in context. You don't search for it—it appears because it's relevant.

**Gap**: Maestro makes context explicit when it should be implicit. Five chips ask users to understand token economics before they've run anything. Power users want precision; new users want "just work."

**Patterns to adopt**:
1. **Smart defaults**: Auto-select context based on what's changed and what the task needs
2. **@ mentions**: `@README.md` or `@src/` to add context inline in the prompt
3. **Collapse chips by default**: Show just the token count; expand to see what's included
4. **Contextual suggestions**: "Your branch has 5 changed files—include them?" instead of always-on toggles

---

## Worktree Sidebar

**Current**: `WorktreeSidebar.swift` shows a "BRANCHES" header, list of worktrees with branch names, commit counts, colored stage badges, and hover actions. Empty state explains worktrees. Pipelines and Agents sections appear for beta users.

**Inspiration—Notion**: Notion's page tree is effortlessly navigable. Drag to reorder, indent for hierarchy, icons show page type at a glance. The structure is visible but unobtrusive.

**Inspiration—Figma**: Figma's layers panel shows what exists without demanding attention. Hover reveals actions. Selection is clear. The panel feels like a mirror of the canvas, not a separate interface.

**Gap**: "BRANCHES" uses git jargon that non-engineers don't understand. The sidebar explains worktrees only in empty state—once you have one, the explanation disappears. Stage badges use color alone. Hover actions crowd four icons into a tiny space.

**Patterns to adopt**:
1. **Rename header**: "BRANCHES" -> "AI WORKSPACES" or "FEATURES IN PROGRESS"
2. **Persistent help**: Add a small "?" icon that shows explanation on hover/click
3. **Reduce hover actions**: Show only the most common (Terminal, PR) on hover; move others to context menu
4. **Stage badges with icons**: design=lightbulb, implement=hammer, review=magnifier, polish=sparkles

---

## Running State

**Current**: When a task runs, Maestro launches an external terminal. The sidebar shows a pulsing blue dot. `ResultsPanel.swift` shows "Running {task}..." with a spinner and elapsed time. Live output can be toggled to show a streaming log.

**Inspiration—Cursor**: Cursor streams output inline. You see the agent thinking, writing, iterating. The streaming feels like watching someone type—immediate connection.

**Inspiration—Figma**: Figma's presence indicators show where collaborators are and what they're doing. You know the state of the system at a glance.

**Gap**: Results appear in an external terminal, not in Maestro. Users must switch apps to see what's happening. The results panel shows a summary after completion but not live progress. The pulsing dot is the only in-app indication that anything is happening.

**Patterns to adopt**:
1. **In-app streaming**: Show agent output live in the results panel, not just after completion
2. **Progress phases**: Show what step the agent is on (reading files, writing code, running tests)
3. **Notification on completion**: System notification when background task finishes—don't require terminal watching
4. **Quick-open terminal**: One-click to jump to the terminal if users want the full output

---

## Errors/Empty States

**Current**: Empty worktree state shows an icon, "No worktrees yet", explanation text, and a "Create Worktree" button. Errors use standard SwiftUI alerts with "OK" button.

**Inspiration—Notion**: Notion's empty pages feel like opportunities, not voids. "Press Enter to continue with an empty page, or pick a template, or import..." Every empty state offers a path forward.

**Inspiration—Figma**: Figma's errors are specific and actionable. "Can't connect to font server—use local fonts instead?" instead of "Something went wrong."

**Gap**: Maestro's empty state is good for worktrees but missing elsewhere. Error messages like "Failed to delete worktree: {error}" are developer-speak. There's no guidance on what went wrong or how to fix it.

**Patterns to adopt**:
1. **Empty prompt state**: Instead of just placeholder text, show "Try these:" with example prompts
2. **Empty results state**: "No recent runs" with "Run your first task" button
3. **Actionable errors**: "Couldn't create worktree—branch name already exists. Try a different name."
4. **Recovery paths**: Offer next steps, not just dismissal buttons

---

## Summary: Priority Gaps

1. **Results appear in external terminal** - Impact: High
   - Users lose context switching between apps
   - New users don't know to check terminal
   - No notification on completion

2. **No onboarding or progressive disclosure** - Impact: High
   - All options visible immediately (5 context chips, model selector, voice selector)
   - New users don't know what tasks do
   - Git jargon ("worktree", "branch") unexplained

3. **Task selector requires explicit choice** - Impact: Medium
   - "Select task..." gives no guidance
   - Mode labels (auto/interactive) meaningless to newcomers
   - No default or smart inference

4. **Context controls are explicit, not implicit** - Impact: Medium
   - Five toggles ask users to understand token economics
   - No @ mentions for surgical context addition
   - Power users want precision; new users want "just work"

5. **Command preview hidden by default** - Impact: Medium
   - Power users want to see what's running
   - Builds trust and understanding
   - Hidden behind two clicks (Options -> Command Preview)

6. **Stage badges use color alone** - Impact: Low
   - Fails accessibility for color-blind users
   - Easy fix: add icons

---

## Patterns to Steal

1. **From Cursor—Streaming output inline** - Apply to ResultsPanel
   - Show agent output live in the app, not just terminal
   - Makes the agent feel alive and responsive

2. **From Cursor—@ mentions for context** - Apply to PromptLauncher
   - `@file.ts` to add specific files
   - Faster than checkbox toggles, more discoverable than drag-and-drop

3. **From Notion—Slash commands** - Apply to PromptLauncher
   - `/design`, `/review` as task shortcuts
   - Typing is faster than clicking dropdowns

4. **From Notion—Empty state as opportunity** - Apply everywhere
   - Empty prompts: "Try these: 'add a login page', 'fix the failing test'"
   - Empty results: "Run your first task to see results here"

5. **From Linear—Keyboard-first with visible shortcuts** - Apply globally
   - Show `Cmd+Enter` prominently on Run button (already done)
   - Add `Cmd+K` command palette for all actions
   - Display shortcuts in menus and tooltips

6. **From Figma—Remove friction before adding features** - Apply to onboarding
   - Pre-select sensible defaults (model, context, task)
   - Reduce visible options for new users
   - Show advanced controls only after first successful run

7. **From Stripe—Three-column layout** - Consider for results
   - Navigation (worktrees), Content (prompt), Output (streaming)
   - Each column has a clear purpose and can be collapsed

8. **From Linear—Opinionated defaults** - Apply to context toggles
   - Include Docs + Files by default, hide Diff/Clipboard/Summaries
   - Let users customize after they understand the system

---

## Wild Ideas (Artist Mode)

Why does Maestro even need to be an app? What if it was:

1. **A Raycast extension**: Cmd+Space, type "lf implement: add auth", see worktrees in sidebar. The app is just a status tray icon.

2. **A menu bar agent**: Always visible, always ready. Click to see running tasks. Right-click for quick actions. The full app is optional.

3. **A conversational interface**: No dropdowns, no toggles. Just a text field. "run implement on a new branch" -> AI figures out the rest.

4. **A diff-first interface**: Instead of "prompt -> results", show "current state -> proposed state". The prompt is secondary; the outcome is primary.

5. **Agents that suggest work**: Instead of you picking tasks, Maestro notices "your main branch is 50 commits ahead" and offers to run polish.

The current design assumes users know what they want and how to ask for it. What if Maestro was opinionated enough to propose actions based on context?
