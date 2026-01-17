# UX Gap Analysis

Analysis of Maestro against best-in-class tools: Figma, Cursor, and Notion.

This revision questions fundamental assumptions and proposes more radical improvements.

---

## The Core Problem

Maestro is a GUI wrapped around a CLI. The design assumes users already understand loopflow's concepts (worktrees, tasks, pipelines, voices) and just want buttons instead of commands. This is backwards.

**What if Maestro wasn't a CLI GUI?**

Figma doesn't teach you design theory before letting you draw. Notion doesn't explain databases before giving you a page. Cursor doesn't require understanding of context windows before you can ask a question.

The most delightful tools let you *do something useful immediately*, then reveal depth as you need it.

---

## Welcome/Setup

**Current**: SetupView shows a progress stepper installing dependencies. WelcomeWindow shows recent repos with a generic tagline about worktrees and coding sessions.

**Why does it have to be this way?**

It doesn't. The setup flow assumes:
1. Users must install CLI tools before using Maestro
2. Users care about seeing what's being installed
3. The first experience should be configuration, not value

**Radical alternative: Demo-first onboarding**

What if the welcome screen showed a *live demo repository* with fake worktrees, example prompts, and simulated output? Not a tutorial video—an interactive sandbox you can click around in.

*Figma* lets you play with a template before signing up. *Notion* gives you example pages to edit immediately. *Cursor* opens your code and lets you ask questions within seconds.

**Pattern to steal: Notion's template gallery**

Instead of "Open Folder...", the welcome screen should be a gallery:
- "Start from scratch" → open folder picker
- "Try with example repo" → opens bundled demo
- Recent repos below, not above—they're for returning users, not first impressions

**Unanswered question**: What if Maestro could run without the lf CLI for basic operations? A bundled mini-runtime that degrades gracefully?

---

## Prompt Input

**Current**: Task selector dropdown + large TextEditor + Run button. Context options hidden behind "Options" toggle.

**Why does it have to be this way?**

The current design treats task selection and prompt writing as separate steps. But they're not—they're one continuous act of expressing intent.

**What's the wildly different approach?**

Imagine if the entire input was just a single text field that understood everything:

```
/implement add user authentication @src/auth.py --voice architect
```

Type `/` → task suggestions appear. Type `@` → file picker appears. Type `--` → flag completions appear. The grammar *is* the interface.

This is how Notion's slash commands work. This is how Cursor's @ mentions work. The text field adapts to what you're typing.

**Radical alternative: Conversational prompt builder**

What if there was no task selector at all? You just describe what you want:

> "Review my changes and fix any style issues"

Maestro figures out:
- Task: review
- Mode: auto (it can fix things)
- Context: diff + style guide

The task dropdown becomes a *suggestion* after you type, not a *requirement* before.

**The real gap**: Maestro asks you to translate your intent into loopflow's vocabulary. The best tools translate *for* you.

---

## Context Controls

**Current**: Toggle chips (Docs, Files, Diff, Clipboard) + drag-and-drop zone + token counter.

**Why does it have to be this way?**

The toggle model assumes:
1. Users understand what each toggle includes
2. Categories are the right abstraction
3. Token counts are meaningful information

None of these are obviously true.

**What would make this delightful?**

*Direct manipulation*. Instead of toggling "Files" on/off, what if you saw a minimap of your codebase and could *click* on files to include them? The spatial representation makes context concrete.

Figma's component panel doesn't say "Include components (on/off)". It shows you the actual components you can use.

**Pattern to steal: Cursor's context pills**

In Cursor, when you @ mention a file, it appears as a pill in your message. You can see *exactly* what you're including. Remove a pill → file is excluded. The context is the message.

What if Maestro showed the assembled context in a collapsible preview? Click to expand → see every file, every doc, the exact prompt that will be sent. Make the invisible visible.

**Radical alternative: No explicit context controls**

What if context was entirely automatic? Maestro watches what you type and infers context:
- Mention "auth"? Include files with "auth" in the name.
- Say "fix the bug in"? Wait for a file mention or use recent edits.
- Reference "the style guide"? Automatically include STYLE.md.

The toggle chips become a power-user override, not the primary interface.

---

## Worktree Sidebar

**Current**: Flat list of branch names with status badges and hover actions.

**Why does it have to be this way?**

The sidebar treats worktrees as a list to manage. But worktrees are *work in progress*—they have state, history, and momentum.

**What would make this delightful?**

*Kanban view*. Instead of a flat list, organize by state:

```
┌─────────────┬─────────────┬─────────────┐
│ In Progress │ Ready       │ Shipped     │
├─────────────┼─────────────┼─────────────┤
│ auth-feature│ cleanup-pr  │ v0.6.3      │
│ (implement) │ (reviewed)  │             │
│             │             │             │
└─────────────┴─────────────┴─────────────┘
```

You can *see* your workflow at a glance. Drag a card to a new column to change its state.

**Pattern to steal: Linear's issue views**

Linear lets you switch between list, board, and table views of the same data. The worktree list is just one view—what about a timeline view showing when each branch was last active?

**Radical alternative: No explicit worktree management**

What if worktrees were entirely implicit? You describe what you want to build:

> "Add OAuth login"

Maestro creates a worktree automatically, runs the task, and surfaces the result. You only see worktrees when you want to—most of the time, you just see your *work*.

The git plumbing disappears. You're left with features in various states of completion.

---

## Running State

**Current**: OutputPanel at the bottom with collapsible streaming output. Green dot for running.

**Why does it have to be this way?**

The output panel assumes:
1. Users want to see raw CLI output
2. Spatial separation (input at top, output at bottom) is helpful
3. Text streaming is the right feedback model

**What would make this delightful?**

*Inline progress*. When you click Run, the input transforms:

```
┌─────────────────────────────────────┐
│ ◉ Running: implement auth-feature   │
│                                     │
│ → Reading: src/auth.py              │
│ → Editing: src/routes/login.py      │
│ → Writing tests...                  │
│                                     │
│ [Stop] [View full output]           │
└─────────────────────────────────────┘
```

The output is *in place*. Your eye doesn't need to move. This is how Cursor shows AI responses—in the chat flow, not a separate panel.

**Pattern to steal: Figma's multiplayer cursors**

What if running tasks showed a presence indicator *in the worktree row*?

```
  auth-feature  [Claude is editing...]  🟢
```

The agent becomes a collaborator you can see working, not a background process.

**Radical alternative: No streaming output by default**

What if the default was to show *nothing* during execution? Just:

> "Claude is working on auth-feature. Estimated: 2-3 minutes."

When it's done:

> "✓ auth-feature ready for review. 4 files changed."

The raw output becomes an optional "show work" mode for debugging. Most users don't need to watch the sausage being made.

---

## Errors/Empty States

**Current**: Modal alerts with OK buttons. Empty states show minimal text ("No worktrees").

**What would make this delightful?**

Every empty state should answer: "What should I do?"

**Empty repo state**:
```
┌─────────────────────────────────────┐
│                                     │
│     🌱 Fresh start                  │
│                                     │
│  This repo has no worktrees yet.    │
│                                     │
│  [Start a feature]                  │
│                                     │
│  "add user auth"                    │
│  "refactor database layer"          │
│  "fix the login bug"                │
│                                     │
│  ↑ Try one of these, or type your   │
│    own in the prompt above          │
│                                     │
└─────────────────────────────────────┘
```

The empty state *teaches* by example. No jargon, no explanations of git concepts.

**Pattern to steal: Notion's empty page**

Notion's empty page says "Press Enter to start writing, or choose a template below." The empty state is the onboarding.

**Error recovery**:

Instead of:
> "Error: Failed to create worktree"
> [OK]

Show:
> "Couldn't create worktree. The branch name 'main' is reserved."
> [Try a different name] [Learn about worktrees]

Errors should offer next actions, not dead ends.

---

## Summary: Priority Gaps

1. **No zero-config value demonstration** - Impact: Critical
   - Users must configure before experiencing. Demo repo/sandbox would flip this.

2. **Prompt input doesn't understand intent** - Impact: High
   - Separate task selector + args fragments the mental model. Unified intelligent input would feel magical.

3. **Context is invisible until executed** - Impact: High
   - Users toggle categories, not files. Preview of assembled context would build trust.

4. **Output is spatially disconnected** - Impact: High
   - Input at top, output at bottom breaks flow. Inline progress keeps attention focused.

5. **Worktrees are a list, not a workflow** - Impact: Medium
   - Flat list doesn't show progress. Kanban/timeline would reveal work state.

6. **Empty states don't teach** - Impact: Medium
   - "No worktrees" says nothing about what to do. Example prompts would bootstrap understanding.

7. **No stop button** - Impact: Medium
   - Can't cancel a running task from the UI. Prominent stop affordance needed.

---

## Patterns to Steal

### From Cursor
1. **@ mentions for inline context** → Type `@file.py` in prompt, file appears as pill
2. **Chat-style output** → Response appears inline, not in separate panel
3. **Model selector as afterthought** → Subtle dropdown, not prominent control

### From Notion
1. **Slash commands** → `/review` types in prompt, shows matching tasks
2. **Empty page as onboarding** → "Type / for commands" teaches through doing
3. **Template gallery** → Welcome screen shows what's possible

### From Figma
1. **Presence indicators** → Show "Claude working on..." in worktree row
2. **Playground before commitment** → Demo canvas before creating project
3. **Direct selection** → Click files to include, not toggle categories

### From Linear
1. **Multiple views of same data** → List/board/timeline for worktrees
2. **Keyboard-first design** → Power users never touch mouse
3. **Progressive disclosure** → Simple by default, complex when needed

---

## Design Principles Revisited

### Bret Victor: "Create by reacting"

The current flow: think about what task → select task → configure context → type args → run → wait → see result

The reactive flow: type what you want → system suggests task → shows preview → run → see result inline

**Gap**: Too much planning required before seeing any feedback.

### Norman: Affordances

**Problem**: The TextEditor for prompt input has no border, no placeholder examples, no indication of the grammar it accepts. It looks like a display field, not an input.

**Fix**: Syntax highlighting as you type. `/implement` turns blue. `@file.py` becomes a clickable pill. The input *shows* what it understands.

### Ive: Simplicity through removal

**Problem**: Task selector, mode picker, voice selector, context toggles, token count, options toggle, run button—seven controls before you can type your prompt.

**Fix**: Start with just the text input and run button. Other controls appear when you demonstrate need (type `--voice`) or can be inferred from context.

---

## The Heretical Question

Why does Maestro need to be a macOS app at all?

A VS Code extension or web app would:
- Have zero installation friction
- Live where developers already work
- Integrate with file trees, terminals, and diff views natively

The macOS app makes sense if the value proposition is *orchestration across multiple editors/terminals*. But current Maestro mostly launches things in external apps anyway.

This isn't a suggestion to rebuild—it's a prompt to clarify: what does the native app uniquely enable? The answer should shape every design decision.

---

## Open Questions

Captured for `.design/questions.md`:

1. Could Maestro work without the lf CLI for basic operations?
2. What would a demo/sandbox repo contain to demonstrate value?
3. Is the worktree abstraction necessary for first-time users, or could it be hidden entirely?
4. Should model selection happen before, during, or after prompt composition?
5. What's the core value of a native macOS app vs. extension/web app?
