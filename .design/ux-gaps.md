# UX Gap Analysis

Comparing Maestro against best-in-class tools: Figma, Cursor, Notion, Linear, and Stripe.

---

## Welcome/Setup

**Current**: WelcomeWindow shows "Loopflow Maestro" with subtitle "Manage worktrees and launch LLM coding sessions." Recent repos list and "Open Folder" button. SetupView requires sequential installation of "Loopflow CLI" and "Worktrunk (wt)" with minimal explanation.

**Inspiration**:
- **Figma**: Opens directly to work. No tutorial dialogs. Professional trust.
- **Notion**: Templates gallery—start with something useful, not a blank slate.
- **Cursor**: Just works. Dependencies handled silently or explained in context.

**Gap**: Maestro front-loads jargon ("worktrees", "LLM coding sessions") and requires understanding of dependencies before showing value. First-time users see barriers before benefits.

**Patterns to adopt**:
1. **Show, don't tell**: Instead of "Manage worktrees and launch LLM coding sessions", show what that *looks like*—a visual of the workflow or a 3-second animation.
2. **Dependency invisibility** (fast.ai): Check/install dependencies silently during first repo open, not as a blocking wizard. Show progress inline, not as a separate view.
3. **Template gallery** (Notion): For repos without `.lf/`, offer starter templates: "Review existing code", "Implement a feature", "Quick fix". One click to first task.
4. **Immediate success path** (fast.ai): A new user should complete their first task within 60 seconds of opening the app.

---

## Prompt Input

**Current**: Task selector (searchable dropdown) + large TextEditor with "What do you want to build?" placeholder. Two entry points: (1) select task from dropdown, (2) type `task: args` in text field. Prompt picker appears when typing matches task names, creating interference between the two patterns.

**Inspiration**:
- **Cursor**: Three distinct tiers—Tab (autocomplete), Cmd+K (inline edit), Agent (chat). Each surface is purpose-built.
- **Notion**: Slash commands (`/`) in a single text field. One entry point, infinite extensibility.
- **Linear**: Cmd+K command palette searches everything. Input → action is a single gesture.

**Gap**: Dual input paths (task selector + colon syntax) create confusion. The prompt picker interrupts natural typing. No command palette for keyboard-first navigation. The "Run" button requires mouse targeting.

**Patterns to adopt**:
1. **Single input, slash commands** (Notion): Remove the Task selector. Use `/` prefix for tasks: `/implement`, `/review`. Text without slash = inline prompt. One field, one mental model.
2. **Cmd+K everywhere** (Linear): Global shortcut opens prompt input focused, ready to type. Same shortcut dismisses. No separate "Task" dropdown.
3. **Typeahead as guidance** (Cursor): When user types `/`, show filterable task list below. When they type freely, show "Will run as inline prompt" confirmation.
4. **Keyboard-first run** (Linear): ⌘↵ already exists, but make it feel *instant*—optimistic UI update, not button press + wait.

---

## Context Controls

**Current**: "Options" expand button reveals context toggles (Docs, Files, Diff, Clipboard) as colored chips. Attached files via drag-drop or file picker. Token count shows single number (e.g., "2.1k") with no breakdown.

**Inspiration**:
- **Cursor**: Context is automatic via codebase indexing. Override with `@file` mentions. Visual token budget shows what's included.
- **Stripe**: Three-column layout—content is visible alongside code examples. Progressive disclosure of complexity.
- **Figma**: Component panel shows exactly what's selected. No hidden state.

**Gap**: Context is opaque. Users toggle blindly—no visibility into what's actually included. Token count is meaningless without breakdown. No way to preview assembled prompt (CLI's `-c` flag has no GUI equivalent). No `@` mentions for surgical file inclusion.

**Patterns to adopt**:
1. **Context preview panel** (Cursor): Expand arrow shows exactly what will be sent—file names, character counts, truncation warnings. Make the invisible visible.
2. **@ mentions** (Cursor): Type `@` in the prompt text to fuzzy-search files/folders. Selected items appear as inline chips. Surgical override without leaving the input.
3. **Token budget visualization** (Cursor): Replace single number with segmented bar: Docs (blue, 1.2k), Files (teal, 3.4k), Diff (green, 0.5k). Tap segments to see contents.
4. **Copy assembled prompt** (CLI parity): Button to copy full assembled context to clipboard for inspection. Power user escape hatch.

---

## Worktree Sidebar

**Current**: Header "WORKTREES" + list. Each row shows branch name, commit count, and status badge (last completed task). Hover reveals action buttons (diff, PR, terminal, IDE). Right-click context menu for Create PR, View PR, Land, Delete.

**Inspiration**:
- **Notion**: Page tree with drag-to-reorder, expand/collapse, inline rename.
- **Figma**: Layers panel shows hierarchy with visibility toggles, selection highlighting, and contextual actions.
- **Linear**: Issues list with inline status changes, keyboard navigation, and batch operations.

**Gap**: No visual hierarchy—flat list doesn't communicate relationships. No running state indicator (which worktrees have active tasks?). Hover actions require mouse precision. No keyboard navigation. "WORKTREES" header uses jargon without explanation.

**Patterns to adopt**:
1. **Running state indicator** (Figma presence): Show animated spinner or pulsing dot on worktree rows with active sessions. "electric-penguin ●" for running vs. "electric-penguin ✓" for idle.
2. **Keyboard navigation** (Linear): ↑/↓ to navigate, Enter to open terminal, Space to toggle selection, D for diff, P for PR actions.
3. **Inline status changes** (Linear): Click status badge to cycle: design → implement → review → polish. Visual workflow progression.
4. **Contextual explanation** (Progressive disclosure): First-time empty state says "Worktrees isolate your work—each feature gets its own folder" instead of "No worktrees / Click + to create one."
5. **Drag to reorder** (Notion): Let users organize worktrees by priority/workflow stage. Persist order.

---

## Running State

**Current**: OutputPanel shows streaming lines with session picker (if multiple). Green dot + "N running" indicator. Fixed 200px height. Expandable/collapsible. Auto-scrolls to bottom.

**Inspiration**:
- **Cursor**: Streaming response appears inline, in the editor, where the user's attention already is. Plan mode shows what will happen before it happens.
- **Figma**: Multiplayer cursors and selection highlights—presence is visible without dedicated UI.
- **Linear**: Progress indicators are subtle—counts update in place, no separate "running" panel.

**Gap**: Output panel competes with terminal for attention. Users watch terminal anyway, making the panel redundant. No plan preview before execution. No integration with the prompt launcher—output feels disconnected from input. Running sessions don't update worktree rows.

**Patterns to adopt**:
1. **Plan before execute** (Cursor): Before running, show a preview: "Will read: 3 files, Write: src/auth.py, Run: tests". User confirms or edits. Transparency builds trust.
2. **Progress in context** (Figma): Instead of separate OutputPanel, show progress *on the worktree row*: subtle animation, percentage, or stage indicator. The sidebar becomes the status dashboard.
3. **Inline streaming** (Cursor): Option to show agent output *in the prompt launcher area* after clicking Run. The response appears where the prompt was typed—immediate connection.
4. **Terminal as escape hatch**: Keep terminal integration, but make it feel like "advanced mode"—not the default destination. Power users can still double-click to open Warp.

---

## Errors/Empty States

**Current**: Errors shown via `alert()` dialogs. Empty worktrees state: "No worktrees / Click + to create one". SetupView errors show red text with "Retry" button. Launch failures show modal alert.

**Inspiration**:
- **Notion**: Empty pages invite action—"Press / for commands" with subtle animation.
- **Figma**: Placeholders are helpful, not apologetic. "Create your first design" with template options.
- **Stripe**: Error states include specific remediation. "API key invalid. Generate a new key →"

**Gap**: Error messages are technical, not actionable. Empty states are uninviting. Modal alerts break flow—user must dismiss before continuing. No inline validation or prevention.

**Patterns to adopt**:
1. **Actionable errors** (Stripe): "Failed to create worktree: branch already exists" → "Failed to create 'auth-feature'—it already exists. Open existing? / Choose different name?"
2. **Inline errors** (Notion): Show errors *where* they occurred—red border on input field, tooltip with explanation. No modal interruption.
3. **Inviting empty states** (Figma): "No worktrees yet" + visual illustration + "Create your first worktree" button + "What are worktrees?" expandable explanation.
4. **Preventative validation**: If branch name already exists, show warning *while typing*, before user clicks Create.
5. **Recovery-oriented design** (Don Norman): Every error should suggest at least one path forward. Never leave users stuck.

---

## Design Principle Violations

| Principle | Current State | Remediation |
|-----------|--------------|-------------|
| **Immediate Connection** (Bret Victor) | Response happens in terminal, not where typed | Inline streaming option; plan preview before execution |
| **Progressive Disclosure** (Notion, Stripe) | Advanced concepts (pipelines, voices, tokens) visible immediately | Hide behind "Options"; reveal on hover/expand |
| **Speed as Feature** (Linear, Figma) | Mode picker, token count, Options button add visual weight | Remove friction from happy path; keyboard-first |
| **Keyboard-First** (Linear) | Task selector requires mouse; no Cmd+K | Global command palette; keyboard nav in sidebar |
| **Transparency** (Cursor) | Context assembly opaque; token count is single number | Context preview; segmented token bar; @ mentions |
| **Remove Barriers** (fast.ai) | Setup requires understanding dependencies | Silent install; inline progress; immediate success |
| **Opinionated Defaults** (Linear) | Mode picker asks Auto vs Interactive without context | Default to Auto; explain only when user hovers/expands |
| **Design Should Disappear** (Jony Ive) | UI chrome visible before content; Options section adds bulk | Minimize visible elements until needed |
| **Craft Signals Care** (Ive, Collison) | Functional but utilitarian; no visual delight | Micro-animations; considered typography; polish |

---

## Summary: Priority Gaps

1. **Mental model mismatch** - Impact: **Critical**
   - Users expect in-app responses; Maestro launches terminal sessions
   - Remediation: Inline streaming option + plan preview before execution

2. **Context opacity** - Impact: **High**
   - Users can't see what context is assembled; toggle blindly
   - Remediation: Context preview panel + @ mentions + segmented token bar

3. **Dual input confusion** - Impact: **High**
   - Task selector + colon syntax compete; prompt picker interrupts typing
   - Remediation: Single input with slash commands; remove Task selector

4. **No keyboard-first navigation** - Impact: **High**
   - Power users slowed by mouse targeting
   - Remediation: Cmd+K command palette; keyboard nav in sidebar

5. **Jargon barrier** - Impact: **Medium**
   - "Worktrees", "LLM coding sessions" assume prior knowledge
   - Remediation: Show-don't-tell; contextual explanations on hover

6. **Setup friction** - Impact: **Medium**
   - Blocking dependency wizard before showing value
   - Remediation: Silent background installation; inline progress

7. **Running state invisible** - Impact: **Medium**
   - Worktree rows don't show active sessions; OutputPanel redundant with terminal
   - Remediation: Progress indicators on sidebar rows; plan preview

---

## Patterns to Steal

1. **Slash commands** (Notion) → Apply to prompt input. `/implement`, `/review`. One field, one model.

2. **@ mentions for files** (Cursor) → Apply to context controls. Type `@src/auth.py` to add surgical context.

3. **Cmd+K command palette** (Linear, Cursor) → Apply globally. One shortcut to do anything.

4. **Plan before execute** (Cursor) → Apply before Run. Show what will happen, let user confirm/edit.

5. **Progress on entity rows** (Figma presence) → Apply to worktree sidebar. Show running state inline, not in separate panel.

6. **Silent dependency handling** (fast.ai) → Apply to setup. Check/install in background, not blocking wizard.

7. **Segmented progress bar** (Cursor token budget) → Apply to token count. Show breakdown by context type.

8. **Templates for cold start** (Notion) → Apply to first-run. Offer starter tasks for repos without `.lf/`.

9. **Actionable error messages** (Stripe) → Apply everywhere. Every error suggests a path forward.

10. **Keyboard shortcuts displayed** (Linear) → Apply to menus and tooltips. Learning mechanism built-in.

---

## Wild Ideas (Question the Constraints)

### What if Maestro wasn't a launcher?

The current architecture—prompt in GUI, output in terminal—creates a mental model mismatch. What if Maestro *was* the terminal? Embed a terminal emulator directly. The prompt launcher becomes the input line. Streaming output appears inline. The worktree sidebar persists. No context switching.

> This is interesting. Id love to have a terminal inside Maestro, but i would love to just literally embed the user's favorite terminal. I think it's scope creep to compete with warp and ghostty, at least for now.

### What if worktrees were invisible?

Worktrees are an implementation detail. Users want "work on this idea in isolation"—not "create a git worktree." What if Maestro auto-created worktrees silently and showed them as "Ideas" or "Experiments"? The git mechanics disappear; the creative metaphor remains.

> Yes, this is progressive disclosure that you're explaining. I dont think obfuscating worktrees with a new term helps -- dont invent new terms if we dont need to.

### What if context was visual?

Token counts and toggles are abstract. What if context was *visual*? A minimap of included files. Highlighted snippets showing what the agent will see. Drag to reorder priority. Direct manipulation instead of toggles.

> Yeah, we definitely need more iteration on how the context is represented.

### What if the prompt wrote itself?

The "What do you want to build?" prompt is intimidating. What if Maestro suggested prompts based on recent git activity? "Looks like you're working on auth—want to implement password reset?" The system infers intent from context.

> Nah Dont liek this

### What if errors were impossible?

Instead of error messages, prevent errors entirely. Branch name field auto-suggests available names. Context toggles show warnings before you exceed limits. The Run button only enables when the prompt is valid. Error handling as design constraint, not afterthought.

> I mean, avoiding errors is good ,yes. 

---

## Next Steps

1. Prototype slash command input (replace Task selector)
2. Build context preview panel
3. Add running state to worktree rows
4. Design Cmd+K command palette
5. User test with Curious Beginner profile

