# UX Gap Analysis

Fresh analysis of Maestro against Figma, Cursor, and Notion—this time questioning whether we're solving the right problems.

---

## The Real Question

The previous analysis asked "How do we make Maestro more like Figma?"

The better question: **What job is someone hiring Maestro to do?**

When I open Figma, I want to *design something*. When I open Notion, I want to *write or organize*. When I open Cursor, I want to *write code with AI help*.

When I open Maestro, I want to... what? "Manage worktrees and launch LLM coding sessions" is the current tagline. But that's implementation, not outcome.

**The job to be done**: "Make progress on my codebase while I do something else."

This reframing changes everything.

---

## Where Best-in-Class Tools Excel

### Figma: Collaborative Creation

Figma understood that design is social. The magic isn't the vector tools—it's watching a cursor named "Sarah" move across your canvas in real-time. Figma made design *observable*.

**What Maestro could learn**: The value of loopflow is running AI agents in parallel. But Maestro hides this. You launch a task, it opens a terminal, you wait. Where's the observability? Where's the "Sarah's cursor" moment where you watch Claude read files and make decisions?

### Cursor: AI as Pair Programmer

Cursor's insight: the AI response should appear *where you're looking*. Inline suggestions. Chat that scrolls with your code. The AI is your colleague, sitting next to you, pointing at lines.

Maestro does the opposite. Click Run → terminal window opens → Claude is now somewhere else. The spatial separation breaks the "pair programming" illusion.

**What Maestro could learn**: The agent should be *present* in Maestro, not launched from it.

### Notion: Starting is Free

Notion's blank page feels inviting, not intimidating. "Press / for commands" is the entire onboarding. You learn by typing, not by reading instructions.

Maestro's blank state feels like a form. Select a task. Configure context. Choose a mode. Set voices. *Then* type. The structure precedes the intent.

**What Maestro could learn**: Let people type first. Infer structure from intent, not the reverse.

---

## The Three Modes Problem

Maestro tries to serve three different interaction modes with one interface:

### 1. Quick Command Mode
"Run review on this branch"
- User knows exactly what they want
- Fastest path: type command, hit enter
- Current UX: Too many clicks (task dropdown, mode picker, etc.)

### 2. Exploration Mode
"Help me figure out what to build"
- User doesn't know what they want
- Needs conversation, not configuration
- Current UX: No conversational path—just forms

### 3. Orchestration Mode
"Run ship pipeline across three worktrees"
- Power user managing parallel work
- Needs visibility into multiple agents
- Current UX: Launches to external terminal, loses track

One interface trying to serve all three = mediocre at each.

**Radical idea**: Three distinct entry points:

```
┌─────────────────────────────────────────────────┐
│                                                 │
│   ⚡ Quick Command          💭 Explore          │
│   "lf review"               Chat with Claude    │
│                             about this repo     │
│                                                 │
│                 🎭 Orchestrate                  │
│                 Manage running agents           │
│                                                 │
└─────────────────────────────────────────────────┘
```

Not tabs—modes. Each optimized for its job.

---

## Gap Analysis by Area

### Welcome/Setup

**Current**: Recent repos list + "Open Folder..." button. Setup installs dependencies.

**The real problem**: The app has nothing to show until you give it a repo. This is the opposite of every consumer app pattern.

**What Figma does**: Community templates visible before login. You can browse, preview, and be inspired without commitment.

**What Notion does**: Interactive demo embedded in marketing site. You're already using it before you "start."

**What Cursor does**: Opens immediately when you open a file. Zero configuration before value.

**Radical alternative**: Ship Maestro with a bundled demo repo. First launch = you're already inside a working project with fake worktrees, example prompts, and simulated (pre-recorded) agent output. The "Open your own repo" is a *second* step, not first.

**Even more radical**: What if Maestro could do something useful without a repo at all? A conversational Claude interface for quick questions, code snippets, explanations. The repo workflow becomes a feature you grow into, not a prerequisite.

---

### Prompt Input

**Current**: Task selector dropdown + TextEditor + context toggles + Run button.

**The real problem**: This is a command builder, not an input. The mental model is "configure then execute" rather than "express then refine."

**What Cursor does**: One text field. Type `@` for files, `/` for commands. The input is intelligent.

**What ChatGPT does**: Just a text field. Everything is natural language. The system figures out intent.

**Gap**: Maestro asks users to speak "loopflow" (tasks, voices, context flags). The best AI products let you speak *human* and translate for you.

**Concrete improvements**:

1. **Natural language first**: Type "review my changes and fix style issues" → Maestro suggests task=review, mode=auto, context=diff+style guide. User confirms or adjusts.

2. **Inline mentions**: Type `@` → file picker appears inline. Type `/` → task picker appears inline. No separate controls.

3. **Preview what will run**: Before clicking Run, show the assembled prompt. "Here's what Claude will see: [expandable preview]". Build trust through transparency.

4. **Remember preferences**: If I always use `--voice architect` with review, default it. Learn from usage.

---

### Context Controls

**Current**: Toggle chips (Docs, Files, Diff, Clipboard) + drag-drop zone.

**The real problem**: Categories are abstractions. Users don't think "include the Diff category"—they think "include the changes I made to auth.py."

**What's actually confusing**:
- "Files" means "files touched by this branch"—not obvious
- "Diff" vs "Files"—what's the difference? (Answer: raw diff output vs full file contents)
- Token count with no reference point—is 14k a lot?

**Patterns to steal**:

1. **Cursor's explicit pills**: When you add context, you see exactly what's included as removable pills. No categories—just files.

2. **Notion's progressive disclosure**: Start simple (just the text input). Power controls appear when you need them.

3. **Figma's direct manipulation**: Don't toggle "components on/off"—show the actual component panel and drag what you want.

**Concrete improvement**: Replace toggles with a collapsible "Context preview" that shows exactly what will be sent:

```
Context: 14.2k tokens  [Expand ▼]

Files (3):
  README.md (420 tokens)
  STYLE.md (1.2k tokens)
  src/auth.py (890 tokens)  [×]

Diff:
  +47 -12 lines across 2 files  [×]

[+ Add file]  [+ Add clipboard]
```

Make the invisible visible.

---

### Worktree Sidebar

**Current**: Flat list of branches with status badges.

**The real problem**: Worktrees are git plumbing. Users care about *work*, not branches.

**What Linear does**: Shows issues by status (Todo, In Progress, Done), not by branch name. The work is front and center.

**What GitHub Projects does**: Kanban view of work items. Drag to change status.

**Radical reframe**: What if the sidebar showed "Work" not "Worktrees"?

```
WORK

In Progress
  ┌─────────────────────────────┐
  │ 🔨 Adding user auth         │
  │    implement → review       │
  │    Claude working...  🟢    │
  └─────────────────────────────┘

Ready to Ship
  ┌─────────────────────────────┐
  │ ✅ Fix login bug            │
  │    Reviewed • 4 files       │
  │    [Create PR]              │
  └─────────────────────────────┘

Shipped
  ┌─────────────────────────────┐
  │ 🚀 Refactor DB layer        │
  │    Merged Jan 14            │
  └─────────────────────────────┘
```

The branch names exist somewhere (tooltip, detail view) but aren't primary. The *work* is primary.

---

### Running State & Output

**Current**: OutputPanel at bottom with collapsible streaming text. Green dot for running.

**The real problem**: Output happens in an external terminal. The OutputPanel only shows daemon events, not the actual Claude session. Spatial disconnect between "start here" and "watch here."

**What Cursor does**: Responses appear inline, in the chat flow. Your eye never leaves the conversation.

**What would be magical**: Watch Claude think inside Maestro.

```
┌─────────────────────────────────────┐
│ 🤖 Claude is working on auth-feature │
│                                     │
│ ▸ Read: src/auth.py                 │
│ ▸ Read: src/routes/login.py         │
│ ▸ Planning changes...               │
│                                     │
│ ● Live  [Stop] [View Full Output]   │
└─────────────────────────────────────┘
```

The activity is *in the app*, not in a terminal window you have to find.

**Implementation reality**: This requires the lf CLI to stream output back to Maestro, not just launch a terminal. Non-trivial architectural change. But it's the gap between "launcher" and "workspace."

---

### Error States

**Current**: Modal alerts with OK button. Setup errors have recovery hints.

**The real problem**: Errors are dead ends. "Failed to create worktree" → OK → now what?

**Pattern from Notion**: Errors are inline, contextual, and suggest next actions.

**Pattern from Stripe**: Error messages explain *why* and *how to fix*, not just what failed.

**Concrete improvements**:

1. **Inline errors**: Instead of modal, show error below the control that failed. Red text, suggested action.

2. **Recovery flows**: "Branch 'auth' already exists" → [Switch to existing branch] [Use different name: ____]

3. **Graceful degradation**: If lf CLI isn't installed, don't block. Show what you can (file browser, prompts) and CTA to install.

---

## Priority Gaps (Revised)

### Critical

1. **No value without setup** — Users must install CLI + open repo before seeing anything useful. Flip this: demo mode first, real projects second.

2. **Output happens elsewhere** — Running a task opens an external terminal. The disconnect breaks the "workspace" feeling. Bringing output inline would transform the experience.

### High

3. **Configuration before expression** — Task selector, mode picker, context toggles—all before typing. Let users type first, configure second.

4. **Context is categorical, not concrete** — Toggle "Files" on/off vs. see exactly which files will be included. Make the invisible visible.

5. **Worktrees are plumbing, not work** — Show progress on features, not branch names.

### Medium

6. **No stop affordance** — Can't cancel a running task from Maestro UI.

7. **No model selector** — CLI's `-m` flag has no GUI equivalent.

8. **No command preview** — Can't see what will execute before running.

---

## Patterns Worth Stealing

### From Cursor
- **@ mentions**: Inline context references in the text field
- **Inline responses**: Output appears where you're looking
- **Model selector as dropdown**: Present but not prominent

### From Notion
- **Slash commands**: `/review` in the input triggers task picker
- **Blank page that teaches**: "Press / for commands"
- **Progressive complexity**: Simple by default, powerful when needed

### From Figma
- **Community-first welcome**: Templates and examples before blank canvas
- **Presence indicators**: See collaborators (agents) working in real-time
- **Direct manipulation**: Click to include, not toggle categories

### From Linear
- **Work, not branches**: Issues by status, not implementation detail
- **Multiple views**: List/board/timeline of the same data
- **Keyboard shortcuts everywhere**: Power users never touch mouse

### From Arc Browser
- **Spaces**: Different contexts for different work modes
- **Command bar**: ⌘K for everything—universal quick action
- **Boosts**: Remember my preferences and apply them automatically

---

## The Heretical Questions (Revisited)

1. **Does Maestro need to be an app, or a mode?**
   - What if it was a VS Code extension that added loopflow commands?
   - What if it was a CLI with a built-in TUI (like lazygit)?

2. **Is worktree management the core value, or a side effect?**
   - Power users value worktrees for isolation
   - New users don't know what worktrees are
   - Could worktrees be invisible until you need them?

3. **Should the output be in Maestro, or is external terminal fine?**
   - Keeping output in terminal = simpler architecture
   - Bringing output in = more integrated experience
   - Hybrid: summary in Maestro, full output available externally?

4. **What if Maestro worked without lf CLI installed?**
   - Bundled minimal runtime for basic operations
   - Graceful degradation vs. hard dependency
   - Trade-off: distribution complexity vs. first-run friction

---

## Design Principles Applied

### Bret Victor: Immediate Feedback

**Gap**: Click Run → wait → see result eventually, somewhere else.

**Fix**: Show estimated time. Show progress steps. Show output inline. Every action has visible, immediate feedback.

### Norman: Affordances

**Gap**: TextEditor looks like display, not input. Context chips don't indicate click-to-toggle.

**Fix**: Visual affordances that scream "interact with me." Border on focused input. Hover states on everything interactive. Cursor changes appropriately.

### Ive: Simplicity Through Removal

**Gap**: Task selector + mode picker + voice selector + context toggles + token count + options toggle + Run button. Seven controls before typing.

**Fix**: Start with text input + Run. Everything else appears when needed or can be inferred. The first run should require zero configuration.

### Krug: Don't Make Me Think

**Gap**: "Worktrees"—a git concept most users don't know. "Voices"—a loopflow concept that needs explanation.

**Fix**: Use plain language. "Parallel features" not "worktrees." "Tone" or "style" not "voices." If you need a tooltip to explain it, the name is wrong.

---

## Open Questions

1. What would a demo/sandbox repo contain? Fake worktrees, example prompts, pre-recorded agent output?

2. How much of lf CLI could be bundled in Maestro to enable standalone operation?

3. Should "Quick Command," "Explore," and "Orchestrate" be three apps, three modes, or one interface?

4. Is bringing agent output into Maestro (instead of external terminal) worth the architectural complexity?

5. What's the iOS/iPad story? The patterns we pick now constrain future platforms.
