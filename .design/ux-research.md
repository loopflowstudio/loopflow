# UX Research: First-Time Prompter Experience

User research for the Maestro first-run experience, focusing on the prompter and worktree flow.

---

## The Curious Beginner

*Heard about "vibe coding" or AI assistants. Never used Claude Code, Cursor, or Copilot.*

### First Impression (0-5 seconds)

Opens Maestro for the first time. Sees `WelcomeWindow`:

- A branch icon and "Loopflow Maestro"
- Subtitle: "Manage worktrees and launch LLM coding sessions"
- Recent Repositories list (if any)
- "Open Folder..." button

**Internal monologue**: "Okay, 'worktrees'... what's a worktree? And 'LLM coding sessions'—I guess that's the AI stuff. The UI looks clean enough. Let me open my project folder."

The welcome screen is minimalist, which is good (principle: Design Should Disappear), but the terminology assumes familiarity. "Worktrees" and "LLM coding sessions" are insider jargon.

### First Action

Clicks "Open Folder..." and selects their project. If dependencies aren't installed, they hit `SetupView`:

- "First-time setup"
- Two items: "Loopflow CLI" and "Worktrunk (wt)"
- "Install Loopflow" button

**Internal monologue**: "First-time setup, okay. What's Loopflow CLI? What's Worktrunk? Do I need these? The button says 'Install Loopflow'—I guess I should click it?"

They click Install. Progress spinner appears. Then "Install Worktrunk" button.

**Friction point**: The Beginner has no idea what these tools do. The descriptions ("Core command-line tool", "Git worktree manager") are accurate but not helpful. They don't explain *why* these are needed or what they enable.

### First Obstacle

After setup, they see the main `ContentView`:

- Left sidebar: "WORKTREES" header, empty state "No worktrees / Click + to create one"
- Center: The prompt launcher with:
  - "Task" selector (shows "None")
  - Big text field with placeholder "What do you want to build?"
  - Mode picker: "Auto" / "Interactive"
  - "Options" expand button
  - Token count (shows some number like "2.1k")

**Internal monologue**: "Okay, there's a text box. 'What do you want to build?'—that's clear. But wait, I need to pick a 'Task' first? What tasks are there? Let me click the dropdown..."

Clicks the Task field. Sees dropdown with:
- "Tasks" section (empty or showing items like "design", "implement", "review")
- "Pipelines" section (hidden if not beta, but if visible: items with step counts)

**Confusion**: "What's a task? What's a pipeline? If I just want the AI to help me code, which do I pick? The placeholder says 'What do you want to build?'—can I just type there without selecting a task?"

They try typing in the main text area: "add a login page"

The prompt picker appears, filtering for matching task names. If there's no "add" or "login" task, the picker disappears.

**Key confusion**: The relationship between the Task selector and the main text input is unclear. Are they independent? Does selecting a task change what you can type? Can you just type freely?

### Recovery

The Beginner muddles through. They type their request, see no task matches, and click "Run" (⌘↵).

**What happens**: Since they're on main branch and have no worktrees, Maestro auto-creates a worktree with a generated name. A terminal opens (Warp by default) and the command runs.

**Internal monologue**: "Wait, it opened a terminal window? And something called 'electric-penguin' appeared in the sidebar? Did my thing work? What's happening?"

The worktree auto-creation is **good**—it removes a barrier. But the Beginner doesn't understand:
1. Why a separate folder was created
2. What the terminal is doing
3. How to see results

### Verdict

**Would they come back tomorrow?** Maybe, but they're not confident they know what happened. The experience was "mysterious" rather than "magical."

### Pain Points

- [ ] Welcome screen uses jargon ("worktrees", "LLM coding sessions") without explanation
- [ ] Setup screen doesn't explain why dependencies are needed
- [ ] Task vs. prompt text relationship is confusing—which matters?
- [ ] Auto-created worktree appears without explanation
- [ ] Terminal output happens "somewhere else"—no immediate connection to what they typed
- [ ] Token count visible but meaningless to newcomers
- [ ] Mode picker ("Auto" vs "Interactive") has no explanation
- [ ] No onboarding flow or tooltip explaining the core loop

---

## The CLI Convert

*Uses `claude` from terminal daily. Trying GUI to see if it's faster/better.*

### First Impression (0-5 seconds)

Opens Maestro, already has CLI tools installed. Sees the main ContentView immediately:

- Worktree sidebar (recognizes this—they use `wt` commands)
- Prompt launcher with Task selector

**Internal monologue**: "Okay, there's my worktrees. Good. And a prompt input. This is basically `lf <task>: <args>` in GUI form. Let me see what tasks I have..."

Clicks Task selector. Sees their tasks from `.lf/`.

**Internal monologue**: "Nice, it found my task files. Shows 'auto' vs 'interactive' mode badges—that maps to my config. The 'ship' pipeline is there too."

### First Action

Selects "implement" task. Sees it populate the taskSearchText field.

**Internal monologue**: "Okay, so the task is selected. Now I type my args in the big text box... but wait."

They start typing in the main text field. As they type, a prompt picker appears showing matching tasks.

**Confusion**: "I already selected a task up top. Why is it showing me the picker again when I type? Is it overriding my selection?"

They type "implement: add authentication". The prompt picker tries to match "implement" again.

**Friction**: The dual entry points (Task selector AND prompt text with colon syntax) create confusion. CLI users know `task: args` format, but the GUI seems to support both approaches—and they interfere with each other.

### First Obstacle

They want to add context files. In CLI: `lf implement -x src/auth.py`

Clicks "Options" to expand. Sees:
- Voice selector
- Context bar with toggles: Docs, Files, Diff, Clipboard
- Attached files area with "+" button

**Internal monologue**: "Docs, Files, Diff—these map to `--diff-files`, `--diff`. Good. But where's my `-x` equivalent? Oh, the '+' button for attached files. Let me drag a file..."

They drag a file onto the context bar. File chip appears.

**Internal monologue**: "Okay, that works. But I can't see what context is actually being included. In CLI I could use `-c` to copy and inspect. Where's that here?"

They look for a way to preview the assembled context. The token count updates (e.g., "14.2k") but there's no breakdown or inspection.

**Missing feature**: No "copy to clipboard" or "inspect context" equivalent. The CLI's `-c` flag shows exactly what's being sent. In Maestro, you trust the toggles blindly.

### Recovery

They proceed with imperfect visibility. Select task, type args, ensure mode is "Auto", click Run.

The terminal opens in the worktree they had selected.

**Internal monologue**: "Good, it ran in my selected worktree. But I still have to watch the terminal—the output panel at the bottom doesn't show anything useful yet. Why have the GUI if I'm still watching terminal output?"

### Verdict

**Would they come back tomorrow?** Probably not for daily work. The GUI adds friction vs. CLI for their workflow. They'd use it for:
- Quick worktree overview (the sidebar is nice)
- Maybe creating PRs with context menu actions

But for the core loop (write prompt, run task), CLI is faster for power users.

### Pain Points

- [ ] Two entry points for task selection (Task selector + colon syntax in text) cause interference
- [ ] No keyboard shortcut to jump directly to text input (like Cmd+K)
- [ ] No way to preview/inspect assembled context before run
- [ ] Token count is a number with no breakdown—can't see what's eating tokens
- [ ] Output panel shows streaming but they're watching terminal anyway
- [ ] No way to see the actual command being constructed
- [ ] Voice selector requires popover instead of quick entry
- [ ] Missing CLI flags: `--parallel`, model racing, `-c` copy mode

---

## The Prompt Explorer

*Has used ChatGPT/Claude web for coding help. Understands prompting but not git worktrees.*

### First Impression (0-5 seconds)

Opens Maestro. Sees welcome screen with "Loopflow Maestro" and "Manage worktrees and launch LLM coding sessions."

**Internal monologue**: "I've used Claude before for coding. This looks like a macOS app for it. 'Worktrees'—I think that's a git thing? Whatever, let me open my project."

Opens their project folder. If setup needed, they install without much concern.

### First Action

Sees the main interface. Left sidebar shows "WORKTREES" with nothing or just one entry.

**Internal monologue**: "The sidebar is mostly empty. There's a big text box in the middle asking 'What do you want to build?' That's familiar—like ChatGPT's input."

They start typing: "help me refactor this function to be more efficient"

**Problem**: The prompt picker appears, trying to match their text to tasks. No matches, picker disappears.

**Internal monologue**: "What was that dropdown? It went away. Okay, I'll just keep typing... but wait, where do I paste my code? In ChatGPT I just paste code into the text field."

They look for a way to add code context. See the "Options" section with context toggles:
- Docs (blue, enabled)
- Files (teal, enabled)
- Diff (green, disabled)
- Clipboard (purple, disabled)

**Internal monologue**: "Clipboard! I'll copy my code and enable that toggle."

They copy their code, enable "Clipboard" toggle. But there's no indication that it worked.

**Confusion**: "Did it include my clipboard? How do I know? The token count changed from 2.1k to 3.4k—maybe that's my code?"

### First Obstacle

They click Run. Nothing visible happens in the app. Then a terminal window opens.

**Internal monologue**: "A terminal? I didn't want a terminal. I wanted to see the response here, like in ChatGPT. Where's the AI's response?"

They stare at the terminal, which shows the claude CLI running with streaming output.

**Fundamental mismatch**: The Prompt Explorer expected an in-app conversation. Maestro is a *launcher* that opens terminal sessions. The mental model is completely different from chat UIs.

### Recovery

They watch the terminal output. The AI is working on their request. The output panel at the bottom of Maestro shows some streaming lines, but it's a secondary view—the real action is in the terminal.

**Internal monologue**: "Oh, so Maestro just starts the AI session and the actual work happens in the terminal? That's... not what I expected. But the AI is responding, so I guess it's working?"

After the task completes, they see a new worktree appeared in the sidebar. The Prompt Explorer doesn't understand what that means.

### Verdict

**Would they come back tomorrow?** Probably not. The mental model mismatch is too large. They expected:
- In-app conversation
- Paste code, get response
- Iterate in place

Instead they got:
- Terminal-based sessions
- Automatic worktree creation
- Git-centric workflow

This isn't what they were looking for. They'll go back to ChatGPT/Claude web unless they specifically want to learn the worktree workflow.

### Pain Points

- [ ] No explanation that this is a launcher, not a chat UI
- [ ] Clipboard inclusion has no visual confirmation
- [ ] Response appears in terminal, not in-app
- [ ] Worktrees created without understanding
- [ ] No way to iterate/continue conversation from Maestro
- [ ] Output panel is secondary to terminal—doesn't feel "immediate"
- [ ] No "paste code here" affordance like chat UIs have

---

## The Skeptic

*Experienced engineer, skeptical of AI hype. Low patience for jank or hand-holding.*

### First Impression (0-5 seconds)

Opens Maestro. Sees welcome screen.

**Internal monologue**: "Another AI coding tool. Let's see if this is actually useful or just fancy chrome."

Opens their project. Dependency check passes (they already have everything installed for other reasons).

Sees main interface:
- Worktree sidebar with their existing worktrees
- Central prompt launcher

**Internal monologue**: "Clean UI. No tutorial popups, no onboarding wizard. Good—respect my time."

### First Action

Scans the interface to understand the data model:
- Sidebar: "WORKTREES" section lists branches they recognize
- Each worktree row shows: branch name, commit count, status badge
- Central area: Task selector, prompt text, mode picker, options

**Internal monologue**: "Okay, worktrees are just my git worktrees. The task selector has my `.lf/` tasks. This is a GUI for the `lf` CLI. Makes sense."

They select a worktree from the sidebar. The prompt launcher targets it.

**Internal monologue**: "Selected worktree updates the context. Good mental model. Now let me run something real."

### First Obstacle

They select "review" task, leave text empty (review doesn't need args), click Run.

Terminal opens. Claude starts reviewing their branch.

**Internal monologue**: "Fine. But I'm still watching a terminal. What does the GUI add? Let me see..."

They look at output panel in Maestro. Shows streaming lines from the task.

**Internal monologue**: "Streaming output is duplicated from terminal. Not particularly useful—I could just watch the terminal."

They look at worktree sidebar. The worktree row hasn't changed during execution.

**Internal monologue**: "No progress indicator on the worktree row. How do I know which worktrees have running tasks at a glance?"

### Recovery

The Skeptic understands the system quickly—it's a GUI wrapper for CLI tools they know. But they're evaluating whether the GUI adds value.

**Value assessment**:
- Worktree overview: Useful. Seeing all branches with status in one place.
- Quick actions on hover (diff, terminal, IDE buttons): Useful.
- Context menu (Create PR, View PR, Land): Very useful—PR management in one click.
- Prompt launcher: Marginally useful. Typing `lf review` in terminal is equally fast.
- Output panel: Not useful—terminal is primary.

### Verdict

**Would they come back tomorrow?** Selectively. They'll use Maestro for:
- Worktree management (the sidebar is genuinely faster than `wt list` + mental tracking)
- PR operations (right-click → Create PR is convenient)
- Maybe when they want the visual overview

They won't use it for:
- Prompt launching (CLI is faster)
- Watching task output (terminal is better)
- Complex prompts with many context options (CLI flags are more precise)

### Pain Points

- [ ] No visual indication of which worktrees have running tasks
- [ ] Output panel is redundant with terminal—offers no unique value
- [ ] Task selector adds latency vs. just typing `lf <task>`
- [ ] Options panel is mouse-heavy; CLI is keyboard-only
- [ ] Token count is opaque—can't see what's included
- [ ] No diff viewer integrated with prompt launcher (have to right-click worktree separately)
- [ ] Performance feels acceptable but not "sub-100ms" fast per design principles

---

## Summary

### Top 3 Issues Across All Profiles

1. **Mental Model Gap**: Maestro is a launcher, not a chat UI. The Prompt Explorer and Curious Beginner expect in-app responses. The transition to terminal feels like "something went wrong" rather than "working as designed."

2. **Context Opacity**: Users can't see what context is being assembled. Token count is a single number with no breakdown. The CLI's `-c` flag (copy and inspect) has no equivalent. Users toggle options blindly.

3. **Dual Input Confusion**: The Task selector and the colon syntax in the text field compete. Users don't know which input matters. The prompt picker appearing while typing adds noise.

### Secondary Issues

4. **Worktree Jargon**: "Worktrees" appears without explanation. First-time users don't understand why their request created a folder called "electric-penguin".

5. **Output Panel Value**: The streaming output panel duplicates terminal output but offers no unique value. It's not faster, not more detailed, not actionable.

6. **No Onboarding**: No tooltips, no first-run guidance, no progressive disclosure of concepts. Users are dropped into a specialized tool without orientation.

### Design Principle Violations

| Principle | Violation |
|-----------|-----------|
| **Immediate Connection** (Bret Victor) | Response happens in terminal, not where user typed. No immediate feedback in the main UI. |
| **Progressive Disclosure** (Notion, Stripe) | Advanced concepts (worktrees, pipelines, voices) visible immediately without context. No layered reveal. |
| **Transparency** (Cursor, Matuschak) | Context assembly is opaque. Token count is a number, not a breakdown. Can't see what will be sent. |
| **Remove Barriers** (fast.ai) | Setup requires understanding dependencies. Worktrees require git knowledge. No immediate success path. |
| **Opinionated Defaults** (Linear) | Mode picker asks users to choose Auto vs Interactive without explaining implications. Context options are toggles without guidance. |

### Recommendations for Next Phase

1. **Add context preview**: Button to show exactly what will be sent (like `-c` in CLI)
2. **Consolidate task input**: Either Task selector OR colon syntax, not both simultaneously
3. **In-app output parity**: Make output panel valuable—maybe show structured progress, not raw terminal streams
4. **First-run orientation**: Brief explanation of the Maestro model (launcher → terminal) when worktrees sidebar is empty
5. **Worktree explainer**: When auto-creating a worktree, show a brief message: "Created worktree 'electric-penguin' to isolate this work"
