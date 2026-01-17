# UX Gap Analysis

Comparing Maestro against best-in-class tools: Figma, Cursor, Notion, Linear, and Stripe.

**Last updated**: After running state and keyboard shortcuts implementation

---

## What's Been Fixed

Since initial research:

- **Welcome screen jargon** → Now says "AI coding assistant for your projects"
- **Setup explanations** → Descriptions explain benefits, not just tool names
- **Empty worktree state** → Explains isolation concept with visual icon
- **Sidebar header** → Changed to "BRANCHES" with tooltip
- **Mode picker** → Has tooltip explaining Auto vs Interactive
- **Prompt placeholder** → "Describe what you want to build or change..."
- **Error messages** → More actionable with recovery suggestions
- **Context preview panel** → Implemented with expandable sections, file removal, copy button
- **Task typeahead** → Task selector now has search with dropdown
- **Running state indicator** → Worktree rows show pulsing blue dot when task is running
- **Keyboard shortcut** → Cmd+L focuses prompt input (visible in Edit menu)

---

## Remaining Priority Gaps

### 1. Mental Model Mismatch - Impact: **Critical**

**Current**: User types prompt in Maestro, clicks Run, terminal window opens. Results appear in terminal and worktree folder. The app that received input is not the app that shows output.

**Why it matters**: This is the fundamental architectural tension. Users expect ChatGPT-style: type here, response appears here. Instead, Maestro is a launcher that opens a different app.

**Inspiration**:
- **Cursor**: Response streams inline, in the editor where you typed
- **Warp**: Commands and outputs live in the same scrollable view
- **Claude web**: Response appears directly below your input

**The honest question**: Should Maestro compete with terminals, or embrace being a launcher?

**Options**:
1. **Embrace launcher role**: Make the handoff explicit and elegant. Show "Your request is running in Terminal" with a live status badge on the worktree row. Add a notification when complete.
2. **Embed terminal**: Use SwiftTerm or similar to show output inline. Risk: competing with Warp/Ghostty.
3. **Show results summary**: Don't stream the whole output—just show "3 files changed, 1 test added" with a diff preview when done.

**Recommendation**: Option 3. Don't try to be a terminal. Show what matters: what changed, whether it worked, what to do next. The OutputPanel should become a results panel, not a log viewer.

---

### 2. Dual Input Still Confusing - Impact: **High**

**Current**: Task selector dropdown + text field with `task: args` colon syntax. Typeahead search helps, but two paths to the same destination with different behaviors.

**What happens**:
1. User types in text field
2. If text matches a task name, prompt picker appears
3. User can also use task selector dropdown
4. Not clear which one "wins" or how they combine

**Inspiration**:
- **Notion**: Single text field. Type `/` for commands, otherwise it's content. One field, one mental model.
- **Linear**: Cmd+K is the only input. Type what you want, it figures out the rest.

**The real question**: What are we actually selecting?

A task is just a prompt file. The user is saying either "use this prompt" or "use my words directly." Two modes, not two inputs.

**Proposal**: Remove the dropdown. Make the text field the only input.
- Default: Your words become the prompt
- Type `/` at start: Shows task picker inline (Notion-style)
- Selected task appears as a pill/chip above the text field
- Everything else stays the same

---

### 3. No Keyboard-First Navigation - Impact: **High**

**Current**: No Cmd+K command palette. Sidebar requires mouse. Focus switching requires clicking.

**What power users expect**:
- `Cmd+K` → Command palette (run task, open worktree, create PR, show diff)
- `Cmd+1/2/3` → Switch focus (sidebar, prompt, output)
- `↑/↓` in sidebar → Navigate worktrees
- `Enter` on worktree → Open in terminal
- `D` → Show diff
- `P` → PR actions

**Inspiration**:
- **Linear**: Everything searchable from Cmd+K. Shortcuts shown in menus.
- **Figma**: Cmd+/ opens command palette. Every menu shows shortcuts.

**Simpler alternative**: Maybe Maestro doesn't need a full command palette—just excellent shortcuts. The app has ~20 actions, not hundreds.

---

### ~~4. Running State Invisible~~ - **FIXED**

~~**Current**: Worktree rows show static status badge from last task. No indication when a task is actively running.~~

**Implemented**: Worktree rows now show a pulsing blue dot when a session is running. Session events include worktree path, and AppState tracks active worktree paths. The animation uses scale and opacity for a subtle attention-grabbing effect.

---

### 4. Output Panel Redundant - Impact: **Medium**

**Current**: OutputPanel streams lines from running sessions. But the user is already watching the terminal—two views of the same data.

**Question**: What would make this panel worth keeping?

**Ideas**:
1. **Summarize, don't stream**: "Reading 5 files... Planning... Writing src/auth.py..."
2. **Results view**: Show diff preview, test results, what changed. Not a log—an outcome.
3. **Delete it**: If Maestro is a launcher, output belongs in the terminal.

**Recommendation**: Transform into results panel. After task completes, show:
- Files changed (clickable to view diff)
- Tests run (pass/fail count)
- "Open in Terminal" button for full logs

---

## Gaps by Area

### Welcome/Setup

**Status**: Mostly fixed. Descriptions are helpful.

**Remaining**:
- [ ] Silent background install instead of blocking wizard
- [ ] Template gallery for repos without `.lf/` config

### Prompt Input

**Status**: Improved with typeahead. Dual-input confusion remains.

**Remaining**:
- [ ] Single input with `/` commands (remove task selector)
- [ ] Keyboard shortcut to focus prompt (Cmd+L or similar)

### Context Controls

**Status**: Context preview panel implemented and working.

**Remaining**:
- [ ] @ mentions in prompt text for surgical file inclusion
- [ ] Segmented token bar showing breakdown visually

### Worktree Sidebar

**Status**: Header changed, empty state improved, actions work.

**Remaining**:
- [ ] Running state indicator (pulsing dot)
- [ ] Keyboard navigation (↑/↓, Enter, D for diff)
- [ ] Drag to reorder

### Running State

**Status**: OutputPanel exists but duplicates terminal.

**Remaining**:
- [ ] Progress indicator on worktree rows
- [ ] Results summary view instead of streaming log

### Errors/Empty States

**Status**: Error messages improved.

**Remaining**:
- [ ] Inline validation (branch name availability while typing)
- [ ] Toast for auto-worktree creation

---

## Design Principle Scorecard (Updated)

| Principle | Before | After | Notes |
|-----------|--------|-------|-------|
| **Immediate Connection** | Gap | Gap | Still sends to terminal |
| **Progressive Disclosure** | Gap | Fixed | Options collapse, preview expands |
| **Speed as Feature** | OK | OK | Feels responsive |
| **Keyboard-First** | Gap | Gap | Still needs work |
| **Transparency** | Gap | Fixed | Context preview shows what's included |
| **Remove Barriers** | Gap | Improved | Setup clearer, still blocking |
| **Opinionated Defaults** | OK | OK | Auto mode default |
| **Design Should Disappear** | Gap | Improved | Options collapse |

---

## Patterns to Steal (Priority Order)

1. **Slash commands** (Notion) → Replace task selector with `/` prefix
2. **Results summary** (GitHub PR checks) → Replace streaming panel with outcome view
3. ~~**Running state on rows** (Figma) → Pulsing indicator on active worktrees~~ ✓ Done
4. ~~**Keyboard shortcuts shown** (Linear) → Display in menus, tooltips~~ ✓ Started (Cmd+L)
5. **Toast notifications** (Notion) → Auto-worktree creation, task completion

---

## Wild Ideas (Questioning Constraints)

Previous wild ideas and responses preserved:

### What if Maestro wasn't a launcher?

Embed terminal directly. Prompt launcher becomes input line. Streaming output inline.

> "This is interesting. Id love to have a terminal inside Maestro, but i would love to just literally embed the user's favorite terminal. I think it's scope creep to compete with warp and ghostty, at least for now."

### What if worktrees were invisible?

Auto-create silently, show as "Ideas" or "Experiments" instead of git terminology.

> "Yes, this is progressive disclosure that you're explaining. I dont think obfuscating worktrees with a new term helps -- dont invent new terms if we dont need to."

### What if context was visual?

Minimap of included files. Highlighted snippets. Drag to reorder priority.

> "Yeah, we definitely need more iteration on how the context is represented."

### What if errors were impossible?

Prevent errors entirely. Auto-suggest available branch names. Show warnings before limits exceeded.

> "I mean, avoiding errors is good, yes."

---

## New Wild Ideas

### What if Maestro was a menubar app?

The prompt launcher doesn't need a full window. Click menubar icon → prompt field appears → type → Enter → window opens only to show results.

**Benefits**: Always accessible. Doesn't compete for screen space. Feels lightweight.

### What if there was no Run button?

Cursor doesn't have Run buttons. You type, it acts. What if pressing Enter runs immediately? No separate "compose then run" step.

**Risk**: Accidental runs. **Mitigation**: Cmd+Enter, or require prompt to end with question mark to distinguish from typing.

### What if the sidebar was just tabs?

Browser model: Each worktree is a tab at the top. Click tab → see that worktree's context. No sidebar.

**Benefits**: More horizontal space for prompt. Familiar metaphor.

### What if Maestro was a Spotlight clone?

Cmd+Space opens floating bar. Type task, hit Enter, it runs. Bar disappears. Notification when done.

No window, no sidebar, no options. Just: what do you want to build?

**This is the ultimate "design should disappear" implementation.**

---

## Summary: What to Build Next

**Must fix** (blocks core flow):
1. ~~Running state indicator on worktree rows~~ ✓ Done

**Should fix** (significant improvement):
2. Unify input with `/` commands, remove task selector dropdown
3. Results summary panel instead of streaming log
4. ~~Basic keyboard shortcuts~~ ✓ Cmd+L implemented; sidebar nav still needed

**Nice to have**:
5. @ mentions for files in prompt
6. Toast notifications for background events
7. Command palette (Cmd+K)

---

## Next Steps

1. ~~Build context preview panel~~ Done
2. ~~Add running state to worktree rows~~ Done
3. Prototype slash command input (replace task selector)
4. Design results panel (replace OutputPanel)
5. Add keyboard navigation to sidebar
