# UX Gap Analysis

## Visual Issues

### Alignment and Spacing
- [ ] **Task selector label alignment** (`PromptLauncher.swift:113-117`): "Task" label sits inline with the typeahead—this breaks visual grouping. Label-above-input is the standard pattern (Notion, Linear).
- [ ] **Context chip overflow** (`PromptLauncher.swift:944-1007`): Chips have no max-width or wrapping. With many attachments, they extend beyond the viewport.
- [ ] **Empty state vertical drift** (`WorktreeSidebar.swift:168-196`): Uses `maxHeight: .infinity`, so content floats low in tall windows instead of centering optically.
- [ ] **Results panel header density** (`ResultsPanel.swift:64-120`): Five controls compete in one row (status, text, duration, toggle, clear, expand). Hierarchy is flat.
- [ ] **Options section lacks visual grouping** (`PromptLauncher.swift:60-66`): Model selector, voice selector, context bar, and command preview run together without clear separation.

### Typography Hierarchy
- [ ] **Mixed caption sizes**: Inconsistent use of `.caption`, `.caption2`, and custom font sizes throughout. No clear typographic scale.
- [ ] **"BRANCHES" header weight** (`WorktreeSidebar.swift:147-149`): All-caps + `.semibold` reads as aggressive for sidebar chrome. Compare Linear's understated section headers.
- [ ] **Placeholder prominence** (`PromptLauncher.swift:564-572`): The main CTA placeholder uses `.tertiary`—too faded. This is the first thing users see.
- [ ] **Task description truncation** (`PromptLauncher.swift:213-218`): Descriptions truncated to 1 line in dropdown. Valuable context gets cut off.

### Color and Contrast
- [ ] **Disabled state opacity** (`PromptLauncher.swift:886`): `opacity(0.5)` may fail WCAG AA 4.5:1 contrast requirements.
- [ ] **Running state indicator** (`WorktreeRow.swift:659-664`): Pulsing blue dot relies on animation. Accessibility concern for motion-sensitive users.
- [ ] **Stage badges color-only** (`WorktreeRow.swift:688-696`): design=blue, implement=purple, review=orange, polish=green. Color alone fails for color-blind users—no shape or icon variation.
- [ ] **Selected worktree** (`WorktreeRow.swift:527-530`): 15% opacity blue accent is too subtle.

### Visual Clutter
- [ ] **Five context chips visible by default** (`PromptLauncher.swift:947-958`): Docs, Files, Diff, Clipboard, Summaries all shown immediately—overwhelming for new users.
- [ ] **Four hover actions** (`WorktreeRow.swift:593-647`): Diff, PR, Terminal, IDE all appear at once with similar small icons—hard to distinguish quickly.
- [ ] **Token count placement** (`PromptLauncher.swift:740-756`): Embedded in crowded row with mode picker and options button.

### macOS Platform Conventions
- [ ] **Redundant repo name** (`ContentView.swift:66-84`): Shows in both `.navigationTitle()` and toolbar. Pick one.
- [ ] **Fixed sheet width** (`NewWorktreeSheet.swift:756`): 320pt is cramped on larger displays; should scale or have minimum.
- [ ] **No File > Open Recent**: Recent repos only accessible from welcome window, not while working.

---

## Welcome/Setup

**Current**: `WelcomeWindow.swift` shows a centered icon, "Loopflow Maestro" title, "AI coding assistant for your projects" subtitle, recent repos list, and "Open Folder" button. `SetupView.swift` handles first-run dependency installation with a 3-step progress indicator.

**Inspiration—Figma**: You're drawing in under 10 seconds. No "here's what this is" preamble. The interface teaches through interaction.

**Inspiration—Notion**: Empty states offer templates that demonstrate value. "Press / for commands" teaches the core interaction. You're productive before you understand the tool.

**Gap**: Maestro's welcome screen is a repo picker, not an introduction to value. "AI coding assistant" tells users nothing about what actually happens. The setup flow handles dependencies but doesn't explain the workflow.

**Why does it have to be this way?**

It doesn't. The current flow assumes users want to open a repo and then figure things out. What if instead:

- **Demo mode**: Let users run a sample task on a demo repo without any setup. Show what happens when an agent writes code.
- **Micro-tutorial on first launch**: 30 seconds showing: "You type what you want -> AI writes code in an isolated branch -> You review the diff". Concrete, visual, fast.
- **Inline examples in placeholder**: Instead of "Describe what you want to build...", show "e.g., add a login page with email/password auth".
- **No repo required to explore**: Let users browse the task library, read prompt files, understand the workflow before committing to a repo.

**Patterns to adopt**:
1. **Figma-style immediate value**: Skip explanation, show interaction
2. **Notion-style templates**: "Start with a common task" options
3. **Concrete over abstract**: Replace "AI coding assistant" with "Tell it what to build. It writes the code while you keep working."

---

## Prompt Input

**Current**: `PromptLauncher.swift` shows a task dropdown ("Select task..."), large text area with placeholder, mode picker (Auto/Interactive), Run button. Context chips appear below with token count. Command preview is hidden behind "Options" toggle.

**Inspiration—Cursor**: Cursor's chat is immediate. Context is automatic. The prompt area adapts to what you're doing. `@` mentions let you surgically add context without mode switches.

**Inspiration—Notion**: `/` commands are discoverable because they appear as you type. The interaction feels like writing, not operating software.

**Gap**: Maestro requires explicit task selection before typing. The dropdown says "Select task..." with no indication of what tasks do. Users must click "Options" to see what context they're sending.

**Why does it have to be this way?**

What if task selection wasn't a dropdown at all?

1. **Infer from prompt content**: User types "review the auth code" -> Maestro suggests "review" task. No dropdown needed.
2. **Slash commands inline**: Type `/review` and it becomes the task. Typing is faster than clicking.
3. **Default task**: Pre-select "implement" (the most common case). Let advanced users change it.
4. **Single input field**: Task + args in one place, like `git commit -m "message"`. Parse intent from text.

The dropdown is a crutch. It says "I don't know what you want, so you pick from a list." A confident tool would say "I think you want X. Press Enter or tell me otherwise."

**Patterns to adopt**:
1. **Smart defaults**: Pre-select task based on branch state (no changes -> design, changes -> review)
2. **Inline completion**: As user types, suggest matching tasks
3. **Slash commands**: `/design`, `/review` typed in the input
4. **Context preview by default**: Show what's being sent without clicking "Options"

---

## Context Controls

**Current**: Five toggle chips (Docs, Files, Diff, Clipboard, Summaries) plus drag-and-drop file attachment. Token count expands to show breakdown. No `@` mentions.

**Inspiration—Cursor**: Context is automatic and smart. Cursor reads your cursor position, open files, recent edits. You don't toggle "include files"—it just knows. Override is surgical: `@file.ts`.

**Inspiration—Figma**: The component panel shows what's relevant to your selection. You don't search for it.

**Gap**: Maestro makes context explicit when it should be implicit. Five chips ask users to understand token economics before they've run anything.

**Why does it have to be this way?**

The context toggles exist because loopflow has configurable context. But most users don't care about tokens. They care about results.

**Wild idea**: What if there were no toggles? Maestro figures out context automatically:
- Running `review`? Include the diff and changed files.
- Running `design`? Include docs and README.
- Running `implement` on a design doc branch? Include `.design/<branch>.md`.

Power users who want control get `@` mentions. Everyone else gets "it just works."

**Patterns to adopt**:
1. **Smart defaults per task**: Each task knows what context it needs
2. **@ mentions for surgical override**: `@src/auth.ts @README.md`
3. **Collapse by default**: Show just token count; expand to see details
4. **Contextual suggestions**: "Your branch has 5 changed files—include them?" as a one-time prompt

---

## Worktree Sidebar

**Current**: "BRANCHES" header, list of worktrees with branch names, commit counts, colored stage badges, hover actions. Empty state explains worktrees. Pipelines and Agents sections appear for beta users.

**Inspiration—Notion**: Page tree is effortlessly navigable. Drag to reorder, indent for hierarchy, icons show type at a glance.

**Inspiration—Figma**: Layers panel shows what exists without demanding attention. Hover reveals actions. The panel feels like a mirror of the canvas.

**Gap**: "BRANCHES" uses git jargon. The sidebar explains worktrees only in empty state—once you have one, the explanation disappears. Stage badges use color alone. Hover actions crowd four icons.

**Why does it have to be this way?**

The sidebar is organized around git concepts (branches, worktrees). But users care about *work*, not git:
- "What features am I building?"
- "What's the agent working on right now?"
- "What needs my attention?"

**Wild idea**: Rename the whole thing. Not "BRANCHES" but "IN PROGRESS" or "AI WORKSPACES". Each item shows:
- Feature name (from prompt or branch)
- Current stage (design -> implement -> review -> polish)
- Status (running, needs review, ready to merge)

The git machinery is hidden. Users see their work, not the implementation detail.

**Patterns to adopt**:
1. **Rename header**: "BRANCHES" -> "FEATURES" or "IN PROGRESS"
2. **Stage badges with icons**: lightbulb, hammer, magnifier, sparkles
3. **Reduce hover actions to 2**: Terminal + context menu for the rest
4. **Persistent tooltip**: "?" icon that explains worktrees on hover

---

## Running State

**Current**: When a task runs, Maestro launches an external terminal. The sidebar shows a pulsing blue dot. `ResultsPanel.swift` shows "Running {task}..." with spinner and elapsed time. Live output can be toggled.

**Inspiration—Cursor**: Streams output inline. You see the agent thinking, writing, iterating. Feels like watching someone type.

**Inspiration—Figma**: Presence indicators show where collaborators are. You know system state at a glance.

**Gap**: Results appear in an external terminal, not in Maestro. Users must switch apps to see what's happening. The results panel shows a summary after completion but not live progress.

**Why does it have to be this way?**

The external terminal exists because that's how Claude Code and the CLI work. But it breaks the flow. Users launch from Maestro, then have to find Terminal to see progress, then come back to Maestro for results.

**Wild ideas**:

1. **In-app terminal emulator**: PTY embedded in results panel. Never leave Maestro.
2. **Progress phases, not raw output**: Instead of streaming text, show: "Reading files... -> Writing code... -> Running tests...". Summary, not firehose.
3. **Background mode with notification**: Task runs silently. System notification when done: "auth-feature: implement complete. 5 files changed." Click to return.
4. **Picture-in-picture terminal**: Small floating terminal that stays visible while you work on other things.

The technical challenge is real (embedded PTY in Swift). But the UX cost of external terminal is also real: context switches, lost windows, no completion notification.

**Patterns to adopt**:
1. **In-app streaming** (if feasible): Show output live in results panel
2. **Progress phases**: "Step 2/4: Writing code..."
3. **System notification on completion**: Don't require terminal watching
4. **Quick-open terminal button**: One click if users want full output

---

## Errors/Empty States

**Current**: Empty worktree state shows icon, "No worktrees yet", explanation text, "Create Worktree" button. Errors use standard SwiftUI alerts with "OK" button.

**Inspiration—Notion**: Empty pages feel like opportunities. "Press Enter to continue with an empty page, or pick a template..."

**Inspiration—Figma**: Errors are specific and actionable. "Can't connect to font server—use local fonts instead?"

**Gap**: Maestro's empty state for worktrees is good. But empty states elsewhere are missing or generic. Error messages are developer-speak.

**Why does it have to be this way?**

Empty states are opportunities for guidance. Every void should offer a path forward:

- **Empty prompt area**: Not just placeholder text, but "Try these:" with clickable example prompts
- **Empty results panel**: "No recent runs. Run your first task to see results here."
- **No worktrees but on a branch**: "You're on feature-x. Run a task to get started."
- **Error creating worktree**: "Branch name already exists. Try: auth-v2"

Errors should be conversations, not alerts. "Couldn't start terminal—Warp not installed. Install Warp or switch to Terminal in settings."

**Patterns to adopt**:
1. **Actionable empty states everywhere**: Every empty state offers a next step
2. **Specific error messages**: Include the fix, not just the problem
3. **Recovery paths in errors**: Buttons for remediation, not just "OK"
4. **Contextual guidance**: Different empty state based on repo state

---

## Summary: Priority Gaps

1. **Results appear in external terminal** — Impact: High
   - Context switch breaks flow
   - New users don't know to check Terminal
   - No completion notification

2. **No onboarding or progressive disclosure** — Impact: High
   - All options visible immediately
   - Task selector doesn't explain tasks
   - Git jargon unexplained ("worktree", "branch")

3. **Task selector requires explicit choice** — Impact: Medium
   - No default task
   - No inference from prompt content
   - No slash commands

4. **Context controls explicit, not implicit** — Impact: Medium
   - Five toggles demand understanding of tokens
   - No smart defaults per task
   - No @ mentions

5. **Command preview hidden by default** — Impact: Medium
   - Power users want transparency
   - Builds trust when visible
   - Two clicks to see

6. **Stage badges use color alone** — Impact: Low
   - Accessibility failure for color-blind users
   - Easy fix: add icons

---

## Patterns to Steal

1. **From Cursor—Streaming output inline**
   - Apply to: ResultsPanel
   - Show agent output live in-app, not terminal
   - Makes the agent feel present

2. **From Cursor—@ mentions for context**
   - Apply to: PromptLauncher input
   - `@file.ts` to add context inline
   - Faster than checkbox toggles

3. **From Notion—Slash commands**
   - Apply to: PromptLauncher input
   - `/design`, `/review` as task shortcuts
   - Typing faster than dropdown navigation

4. **From Notion—Empty state as opportunity**
   - Apply everywhere
   - "Try these prompts..." when empty
   - "Run your first task..." in results

5. **From Linear—Keyboard-first with visible shortcuts**
   - Apply globally
   - Cmd+K command palette for all actions
   - Shortcuts visible in menus/tooltips

6. **From Figma—Remove friction before adding features**
   - Apply to onboarding
   - Pre-select defaults
   - Show advanced only after first success

7. **From Stripe—Three-column layout consideration**
   - Consider for results
   - Navigation | Prompt | Output
   - Each column collapsible

8. **From Linear—Opinionated defaults**
   - Apply to context toggles
   - Docs + Files by default
   - Hide Diff/Clipboard/Summaries

---

## Wild Ideas (Artist Mode)

### Question: Why is Maestro an app at all?

What if it was:

1. **A Raycast extension**: Cmd+Space, type "lf implement: add auth", worktrees in sidebar. The app is a status tray icon.

2. **A menu bar agent**: Always visible, always ready. Click for running tasks. Right-click for quick actions. Full app is optional.

3. **Pure conversation**: No dropdowns. No toggles. Just a text field. "run implement on a new branch" -> AI figures out the rest.

4. **Diff-first interface**: Instead of prompt -> results, show current -> proposed. The prompt is secondary; the outcome is primary.

5. **Proactive agents**: Instead of you picking tasks, Maestro notices "your branch has failing tests" and offers to run polish.

### Question: Why five context toggles?

What if context was:
- **Automatic per task**: design -> docs. review -> diff. implement -> design doc + files.
- **Override only**: No toggles visible. Type `@file.ts` to add. System figures out the rest.

### Question: Why does the sidebar show branches?

What if it showed:
- **Features in progress**: Named by intent, not branch name
- **Agent activity**: What's running, what finished, what needs attention
- **Timeline**: Activity feed like GitHub, not file tree like Finder

### Question: Why launch to external terminal?

What if:
- **Results streamed in-app**: No context switch
- **Terminal was picture-in-picture**: Small floating window
- **Output was summarized, not raw**: "Writing code... 3 files changed... Running tests... All passed"

### Question: Why require a repo to start?

What if:
- **Demo mode**: Try a task on sample code without opening a repo
- **Standalone prompts**: Run a prompt file directly, no repo context needed
- **Clipboard-only mode**: Paste code, get modifications, copy back

---

## The Core Tension

The current design assumes users:
- Know git and understand worktrees
- Want to configure context manually
- Will watch an external terminal
- Know which task they want before typing

What if Maestro was opinionated enough to hide all of that complexity until users ask for it?

**The best tool isn't the most configurable one. It's the one that works without configuration.**

Linear proved this: they refused to build Jira-level complexity. fast.ai proved this: sensible defaults that incorporate best practices. Figma proved this: remove friction before adding features.

Maestro should:
1. **Default to implement** (the most common task)
2. **Auto-select context** based on task type
3. **Stream results in-app** so users never leave
4. **Hide advanced options** until after first successful run
5. **Replace git jargon** with work-centric language

The goal isn't to give users all the options. It's to make the 80% case require zero decisions.
