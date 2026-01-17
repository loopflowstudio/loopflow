# UX Gap Analysis

Analysis of Maestro against best-in-class tools: Figma, Cursor, and Notion.

---

## Welcome/Setup

**Current**: SetupView shows a progress stepper and dependency installation (Loopflow CLI, Worktrunk). WelcomeWindow shows recent repos with folder icons and an "Open Folder..." button. Generic tagline: "Manage worktrees and launch LLM coding sessions."

**Inspiration**:

*Figma*: No setup friction—you're immediately in a canvas. Templates and examples are one click away. The first thing you see demonstrates the product's value.

*Notion*: Pre-populated workspace with example pages. You can delete them, but you *see* what's possible before making any decisions. Templates aren't hidden in a menu—they're the default state.

*Cursor*: Opens directly into your codebase. AI features surface naturally through the interface rather than requiring configuration.

**Gap**: Maestro's welcome feels like an installer, not an invitation. Users must complete setup steps before seeing any value. The recent repos list shows what you've opened, not what you *could* do.

**Patterns to adopt**:

1. **Show before asking**: Instead of "Open Folder...", show a demo repo that's already populated with example worktrees and tasks. Let users explore before committing their own repo.

2. **Productive empty state**: When no repo is open, show the *interface* (greyed out) with example content, so users understand the product before opening anything.

3. **Embedded tutorials**: Figma's "?" corner that expands to contextual tips. Maestro could show "What's a worktree?" or "What's a task?" inline when hovering over empty sections.

---

## Prompt Input

**Current**: PromptLauncher has a task selector dropdown, a large TextEditor with placeholder "What do you want to build?", and a Run button with ⌘↵. Context options are hidden behind an "Options" toggle. Mode selector (Auto/Interactive) is a segmented control.

**Inspiration**:

*Cursor*: Chat input is always visible, minimal chrome. @ mentions for context are inline—you type `@file` and see completions. The model selector is a subtle dropdown, not a prominent control. The input grows naturally with content.

*Notion*: Slash commands (`/`) reveal inline menus without leaving the text flow. You type `/page` and see matching blocks. The interface *adapts* to what you're typing.

**Gap**: Maestro's task selector is a separate control from the input—you pick a task, *then* type args. This is backwards. Cursor lets you type naturally and context emerges from what you write. The "Options" toggle hides critical controls (context, voice) behind a click.

**Patterns to adopt**:

1. **Unified input with inline commands**: Instead of separate task selector + args input, support `/review fix the typo` directly in the text field. Parse the task from the input itself.

2. **@ mentions for context**: Type `@src/` to attach files inline. Show completions as you type. Let context be part of the prompt, not a separate UI layer.

3. **Collapse mode selector**: Auto vs Interactive is a power-user feature. Default to what the task specifies and hide the toggle unless the user explicitly wants to override. Currently it takes up horizontal space for a choice most users don't need to make.

4. **Progressive complexity**: Show just the input by default. If the user starts typing advanced patterns (like `--voice`), surface the relevant controls automatically.

---

## Context Controls

**Current**: ContextChip components (Docs, Files, Diff, Clipboard, Summaries) as toggleable pills. Attached files shown as FileChip with remove buttons. Token counter shows "14.2k" format. Drag-and-drop zone for files.

**Inspiration**:

*Cursor*: @ mentions feel like part of the conversation. Context is additive and visible—you see exactly which files are included because they're in the prompt itself. No toggles to manage; context is declared explicitly.

*Figma*: Component panel shows what's available in the current scope. You don't toggle categories on/off—you pick specific items. The selection is immediately visible.

**Gap**: Maestro's toggle chips are abstract. "Files" means "files touched by this branch" but that's not obvious. Users can't see *which* files are included without running the command. The token counter helps, but there's no breakdown.

**Patterns to adopt**:

1. **Explicit over implicit**: Instead of "Files (on/off)", show the actual file list when hovering. Let users toggle individual files, not categories.

2. **Token breakdown visualization**: Cursor shows token usage per section. Maestro shows a total but not which piece is consuming context. Add a stacked bar or tooltip breakdown.

3. **Inline context declaration**: Support `@README.md @src/utils.py` in the prompt itself, not just as attached files. Make context part of the prompt grammar.

4. **Preview what the agent sees**: One-click to expand the full context assembly. Notion's "expand" concept—click to see what's actually being sent.

---

## Worktree Sidebar

**Current**: WORKTREES section header with + button. List of WorktreeRow components showing branch name, commit count, status badges (design/implement/review/polish), dirty indicator (orange dot), hover actions (diff, PR, terminal, IDE). Empty state: "No worktrees / Click + to create one". Context menu with full action list.

**Inspiration**:

*Notion*: Page tree is hierarchical and draggable. Pages have emoji icons that make navigation scannable. Star/favorite pages float to top. Recent vs. All as filtering modes.

*Figma*: Layers panel shows *everything* in the document. Selection highlights propagate across the UI. Multi-select enables bulk operations. Filter/search within the panel.

**Gap**: Worktree sidebar is flat and unscannable. Every row looks similar—just branch names. No visual hierarchy between active work, waiting work, and completed work. No way to filter, search, or group. The dirty indicator (orange dot) is subtle; users must hover to understand the state.

**Patterns to adopt**:

1. **Status-based grouping**: Section headers like "Running", "Ready for Review", "Clean" that auto-organize worktrees by state. Notion's collapsible sections.

2. **Prominent status indicators**: Instead of colored dots, use full-row backgrounds or left-edge color bars. Make the state immediately visible without hovering.

3. **Search/filter**: Type-to-filter in the sidebar. Figma's instant narrowing—start typing and non-matching items fade.

4. **Emoji support**: Let users assign emoji to worktrees (like Notion pages). Makes the list scannable and personal.

5. **Pinned favorites**: Frequently used worktrees pinned to top, separate from the auto-sorted list.

---

## Running State

**Current**: OutputPanel with collapsible header. Green dot for running, gray for idle. Line count and "N running" text. Auto-scroll to bottom. Color-coded output (→ blue, ✓ green, ✗ red). Session picker dropdown when multiple sessions running.

**Inspiration**:

*Cursor*: Streaming text appears character-by-character in the chat. The AI's "thinking" is visible as it works. Escape or stop button is always accessible. Progress is implicit in the streaming itself.

*Figma*: Presence indicators show who's working where. Cursors move in real-time. Activity is *spatial*—you see where changes are happening.

**Gap**: Maestro's output panel feels disconnected from the prompt launcher. You launch a task, then must expand a different area to see progress. The session picker is confusing when sessions have UUID names. There's no visual connection between a worktree row and its running task.

**Patterns to adopt**:

1. **Inline progress**: Show task progress directly in the worktree row, not in a separate panel. A subtle progress bar or spinner next to the branch name.

2. **Streaming in place**: When you run a task, the output could appear *below* the prompt input, not in a separate expandable area. Keep the user's eye in one place.

3. **Named sessions**: Instead of UUID prefixes, show "review on feature-x" or the task name + branch. Make sessions recognizable.

4. **Cancel affordance**: Prominent stop button during execution. Currently, the only way to stop is to close the terminal or kill the process externally.

5. **Activity presence**: Figma-style avatar dots showing "Claude is working on feature-x". Makes the agent feel like a collaborator, not a background process.

---

## Errors/Empty States

**Current**: Alert dialogs with OK buttons for errors. Empty states show "No worktrees / Click + to create one" in gray text. SetupView shows red error text for installation failures.

**Inspiration**:

*Notion*: Empty pages are inviting, not vacant. "Press Enter to continue..." or template suggestions. The empty state *is* the onboarding.

*Figma*: Placeholders show what *could* be there. Empty frames have dotted outlines. The interface suggests what to do next through its affordances.

**Gap**: Maestro's empty states are dead ends. "No worktrees" tells you nothing about *why* you'd want one or what happens when you create one. Error dialogs require dismissal and don't offer recovery actions.

**Patterns to adopt**:

1. **Actionable empty states**: Instead of "No worktrees", show "Start a feature branch" with one-click creation and a brief explanation. Show *what happens* when you click the button.

2. **Inline errors with recovery**: Instead of modal alerts, show error banners with "Try again" or "Show details" inline. Don't interrupt the user's context.

3. **Example content**: Empty repos could show "Try running `lf design: add user auth` to see how it works" with a copy button. Make the empty state teach.

4. **Graceful degradation**: When the daemon isn't running, show what features are limited instead of failing silently. Offer to start the daemon with one click.

---

## Summary: Priority Gaps

1. **Unified prompt input with inline commands** - Impact: High
   - The disconnect between task selection and prompt input fragments the user's attention. Cursor's single-input model is more natural.

2. **Inline progress and streaming** - Impact: High
   - Separating output into a collapsible panel breaks spatial continuity. Progress should appear where you launched the task.

3. **Actionable empty states** - Impact: High
   - Empty states are dead ends instead of onboarding opportunities. Every empty state should suggest the next action.

4. **Explicit context with @ mentions** - Impact: Medium
   - Toggle chips are abstract. Inline context declaration makes the prompt self-documenting.

5. **Status-based worktree grouping** - Impact: Medium
   - Flat list doesn't scale. Grouping by state (running, clean, dirty) aids scanning.

6. **Cancel affordance** - Impact: Medium
   - No obvious way to stop a running task without going to terminal. Needs prominent stop button.

7. **Welcome experience** - Impact: Medium
   - Setup feels like installation, not invitation. Show value before asking for commitment.

---

## Patterns to Steal

1. **Cursor's @ mentions** - Apply to: Context controls
   - Type `@file.py` inline to add context. Parse context from prompt text itself.

2. **Cursor's streaming in chat** - Apply to: Output panel
   - Output appears where you initiated the action, not in a separate expandable area.

3. **Notion's slash commands** - Apply to: Prompt input
   - `/review` to select task inline. Type flows naturally into command selection.

4. **Notion's empty page experience** - Apply to: Empty states
   - "Press / for commands" in the empty prompt. Template suggestions in empty repo state.

5. **Figma's presence indicators** - Apply to: Running state
   - Show "Claude working..." in the worktree row, not just the output panel.

6. **Figma's component panel** - Apply to: Context controls
   - Show actual files, not categories. Let users pick specific items.

7. **Notion's page emoji** - Apply to: Worktree sidebar
   - User-assigned emoji for worktrees. Makes lists scannable.

8. **Bret Victor's direct manipulation** - Apply to: Context controls
   - Don't toggle "include files"—select specific files directly. See the effect immediately.

---

## Design Principles Assessment

### Affordances (Norman)

**Problem**: ContextChip toggles don't visually suggest they're clickable until you hover. The "Options" toggle is styled like a secondary link, not a disclosure control.

**Fix**: Use platform-standard disclosure triangles. Make interactive elements look interactive through depth, borders, or cursor changes.

### Signifiers (Norman)

**Problem**: The green/orange dots for running/dirty are too subtle. The meaning isn't self-evident—you must learn the color code.

**Fix**: Add labels on hover, or use icons with inherent meaning (spinner for running, pencil for dirty).

### Mapping (Norman)

**Problem**: No spatial relationship between prompt launcher (top) and output (bottom). Running a task on a worktree doesn't visually connect the worktree row to the output.

**Fix**: Output appears adjacent to or within the triggering element. Or use a connecting line/highlight.

### Feedback (Norman)

**Problem**: After clicking "Run", the only feedback is the terminal opening. In the app itself, you must expand the output panel to see progress.

**Fix**: Immediate visual feedback: the Run button becomes a Stop button, progress appears inline, the worktree row shows activity.

### Simplicity (Ive)

**Problem**: Mode selector (Auto/Interactive), Voice selector, and Context options all compete for attention. The interface shows configuration before showing the actual content.

**Fix**: Hide advanced options by default. Surface them progressively as the user demonstrates need (e.g., typing `--voice` shows the voice selector).

### Content over chrome (Ive)

**Problem**: The prompt launcher has considerable chrome (task selector, mode picker, options toggle, run button) before you get to the actual input.

**Fix**: Make the text input dominant. Other controls are supplementary and should recede.

### Direct manipulation (Victor)

**Problem**: Context toggles are indirect—you flip a switch, and somewhere else, tokens are counted differently.

**Fix**: Show the actual context as you toggle. Expand the files included when you enable "Files". Let users see and manipulate the context directly.

### Immediate feedback (Victor)

**Problem**: Token count updates, but you don't see *what* changed. Attaching a file updates the count but doesn't show the file's contribution.

**Fix**: Visual breakdown of token usage. Animate the change when you toggle a context option.
