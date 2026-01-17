# UX Research: User Profiles

User research for the Maestro first-run experience, simulating three user profiles experiencing the prompter and worktree flow.

---

## New Developer

*Just heard about "AI coding assistants". Has never used Claude Code, Cursor, or Copilot. Comfortable with git but not a power user. Wants to understand: "What can this do for me?"*

### First Impression (0-5 seconds)

Opens Maestro for the first time. Sees `WelcomeWindow`:

- A branch icon and "Loopflow Maestro"
- Subtitle: "AI coding assistant for your projects"
- Recent Repositories list (empty on first run)
- "Open Folder..." button

**Internal monologue**: "AI coding assistant—that's what I've been hearing about. Clean interface, not overwhelming. Let me open my project and see what it can do."

The welcome screen is clear. "AI coding assistant for your projects" is immediately understandable without jargon. The branch icon hints at git integration without requiring expertise.

### First Action

Clicks "Open Folder..." and selects their project. Since this is first run, dependencies aren't installed. They see `SetupView`:

- "Loopflow Maestro" header
- "First-time setup" subtitle
- Two items:
  - "Loopflow CLI" — "Runs AI coding tasks from your terminal"
  - "Worktrunk (wt)" — "Keeps each feature in its own folder"
- "Install Loopflow" button

**Internal monologue**: "Setup required. 'Runs AI coding tasks from your terminal'—so it uses the command line. 'Keeps each feature in its own folder'—that's like branches? Makes sense for keeping things organized."

They click Install. Progress spinner. Then "Install Worktrunk" button. They click that too.

**Assessment**: The benefit-focused descriptions help. They understand *why* without needing to know the technical details. The sequential install feels a bit slow but acceptable for first-time setup.

### First Obstacle

After setup, they see the main `ContentView`:

- Left sidebar: "BRANCHES" header with tooltip "Worktrees: isolated folders for each feature branch"
- Empty state with branch icon: "No worktrees yet"
- Explanation: "Each worktree is an isolated folder where AI can work without affecting your main code."
- "Create Worktree" button
- Center: The prompt launcher:
  - "Task" selector showing "None"
  - Text field: "Describe what you want to build or change..."
  - Mode picker: "Auto" / "Interactive" with tooltip
  - "Options" button
  - Token count: "2.1k"

**Internal monologue**: "The sidebar says 'BRANCHES' and explains worktrees. So the AI works in a separate folder—that makes sense, it won't mess up my main code. The text field says 'Describe what you want to build or change'—I'll try typing something."

The empty state explanation is helpful. They understand the isolation concept before creating anything.

They type: "add a user login page with email and password"

**Confusion point**: The Task selector says "None" at the top. Is that a problem? Should they select something first?

**Internal monologue**: "Wait, 'Task: None'—do I need to pick a task? Let me check the dropdown..."

Clicks Task selector. Sees:
- "Tasks" section (might be empty or show "design", "implement", "review")
- Mode badges showing "auto" or "interactive"

**Internal monologue**: "These look like preset prompts? 'implement' sounds right for building a feature. But I already typed my request... does selecting this replace what I typed?"

They select "implement". The task name appears in the selector. Their typed text remains.

**Remaining confusion**: The relationship between selecting a task and typing freely isn't clear. They don't know if both matter or just one.

### Recovery

They click "Run" (⌘↵).

**What happens**: Since they're on main branch with no worktrees, Maestro auto-creates a worktree with a generated name like "curious-dolphin". A terminal window (Warp) opens and shows the Claude agent starting.

**Internal monologue**: "Terminal opened. 'curious-dolphin' appeared in the sidebar—that must be the isolated folder it mentioned. The AI is thinking... this is actually working!"

They watch the terminal. Claude is reading files, making a plan, writing code.

**Internal monologue**: "It's reading my files and writing code. Cool! But why is this happening in a separate terminal window instead of in the app?"

The auto-worktree creation worked smoothly. The terminal transition is jarring but they understand something is happening.

### Verdict

**Would they come back tomorrow?** Yes, with curiosity. The experience worked—AI wrote code and put it in an isolated folder. The terminal output was unexpected but they can see the value.

**What they still don't understand**:
- Why output is in terminal instead of the app
- What the token count means
- When to use tasks vs. just typing
- How to see what the AI actually did (they'd need to open the worktree folder)

### Pain Points

- [ ] Task selector relationship to typed prompt unclear—users don't know if they need both
- [ ] No feedback when worktree is auto-created—appears silently in sidebar
- [ ] Terminal output disconnected from app—unexpected mental model shift
- [ ] No way to see results summary in Maestro—have to open terminal or folder
- [ ] Token count displayed but not explained
- [ ] Can't see what files/context will be included before running

---

## Claude Code Power User

*Uses `claude` CLI daily. Knows prompts, context management, worktrees. Trying Maestro to see if GUI is faster. Expects: feature parity with CLI, plus visual benefits.*

### First Impression (0-5 seconds)

Opens Maestro. Dependencies already installed—sees main `ContentView` immediately:

- Worktree sidebar showing their existing worktrees
- Prompt launcher with Task selector
- Token count in the corner

**Internal monologue**: "There's my worktrees from `wt list`. Tasks from my `.lf/` folder. Token count visible. This is the CLI in GUI form—let's see if it's actually faster."

Clicks Task selector. Sees their custom tasks with mode badges.

**Internal monologue**: "My tasks loaded correctly. Shows auto vs interactive badges matching my config. Good start."

### First Action

Selects "implement" task. Task name appears in selector field.

**Internal monologue**: "Task selected. Now I'll add my args..."

Types in the main text field: "add rate limiting to the API endpoints"

A prompt picker dropdown appears, trying to match their text to task names.

**Friction**: "I already selected the task! Why is it showing me this picker? It's interrupting my typing."

The picker disappears since "add" doesn't match any tasks.

**Internal monologue**: "The GUI supports `task: args` colon syntax AND the task selector. They interfere. In CLI I just type `lf implement 'add rate limiting'`—one clear pattern. Here there are two entry points that conflict."

### First Obstacle

They want to verify context before running. In CLI: `lf implement -c 'add rate limiting'` shows everything.

Clicks "Options" to expand. Sees:
- Voice selector with current selection
- Context toggles: Docs (blue, on), Files (teal, on), Diff (green, off), Clipboard (purple, off)
- Attached files area with "+" button
- Token count updates as toggles change

**Internal monologue**: "Context toggles map to CLI flags. But I can't see *what files* are included. The token count jumped from 2k to 14k when I toggled Files—what's actually in there? In CLI I'd see the full prompt with `-c`."

They look for a "Preview" or "Copy" button. Nothing.

**Missing feature**: No way to inspect assembled context. The CLI's `-c` flag has no GUI equivalent.

**Internal monologue**: "I'm flying blind. I can see token *count* but not token *content*. For a complex task I need to verify the context is right before burning API calls."

### Recovery

They select a worktree from the sidebar, ensure their context toggles are correct, and click Run.

Terminal opens. Claude starts working.

**Internal monologue**: "Terminal output—same as CLI. The output panel in Maestro shows streaming lines too, but it's just duplicating the terminal. Not useful."

They check the worktree row in the sidebar while the task runs.

**Internal monologue**: "The worktree row looks the same as before. No spinner, no indication it's running. If I had three tasks running on different worktrees, I couldn't tell which are active."

### Verdict

**Would they come back tomorrow?** For worktree management, yes. For prompt launching, no—CLI is faster and gives them more control.

**Value assessment**:
- Worktree sidebar: **Useful**. Visual overview beats `wt list`.
- Hover actions (diff, terminal, IDE buttons): **Useful**. Saves typing.
- Context menu (Create PR, View PR, Land): **Very useful**. One-click PR workflow.
- Diff viewer sheet: **Useful**. Better than terminal diff.
- Prompt launcher: **Friction**. Dual entry points, no context preview, slower than `lf implement`.
- Output panel: **Not useful**. Duplicates terminal.
- Token count: **Partially useful**. Need breakdown by section, not just total.

### Pain Points

- [ ] Dual task entry (selector + colon syntax) interfere—pick one pattern
- [ ] No context preview—can't see what files/content will be sent (CLI `-c` equivalent)
- [ ] Token count is single number—need breakdown: Docs 1.2k, Files 5.5k, Diff 0.3k
- [ ] Running state not visible on worktree rows—no spinner or indicator
- [ ] Output panel duplicates terminal without adding value
- [ ] No Cmd+K command palette—keyboard navigation slower than CLI
- [ ] Can't see the actual command being constructed
- [ ] Missing `--parallel` flag for model racing

---

## Designer or PM

*Non-engineer, curious about AI assistance. Might use for docs, specs, or light scripting. Low tolerance for jargon or complexity. Needs: clear affordances, forgiving errors.*

### First Impression (0-5 seconds)

Opens Maestro. Sees `WelcomeWindow`:

- Branch icon and "Loopflow Maestro"
- "AI coding assistant for your projects"
- "Open Folder..." button

**Internal monologue**: "'AI coding assistant'—I've used ChatGPT for writing specs. This looks more serious, maybe for actual code? Let me try it with the project I'm managing."

Opens a project folder. First-time setup appears:

- "Loopflow CLI" — "Runs AI coding tasks from your terminal"
- "Worktrunk (wt)" — "Keeps each feature in its own folder"

**Internal monologue**: "'Terminal'... I don't really use the terminal. But it says 'Install' so I'll click it. 'Keeps each feature in its own folder'—like organizing files?"

They complete setup without fully understanding what they installed.

### First Action

Sees the main interface:

- Sidebar: "BRANCHES" with empty state
- Empty state message: "Each worktree is an isolated folder where AI can work without affecting your main code."
- Center: Big text field "Describe what you want to build or change..."
- Task selector, Mode picker, Options, token count

**Internal monologue**: "The sidebar talks about 'isolated folders' and 'main code'—I'm not sure what that means but okay. The text field is clear: 'Describe what you want'—that's like ChatGPT."

They type: "write a product requirements doc for the new onboarding flow"

**Confusion**: What about the Task selector showing "None"? The mode picker showing "Auto"? What are these?

**Internal monologue**: "Task says 'None'—should I pick something? Let me look..."

Clicks Task selector. Sees items like "design", "implement", "review".

**Internal monologue**: "'design' sounds right for writing a doc. But these seem more like coding tasks? I'll try 'design'."

Selects "design". Then looks at mode picker: Auto vs Interactive.

**Internal monologue**: "'Auto' vs 'Interactive'—what's the difference?"

Hovers over the mode picker. Tooltip appears: "Auto: Runs to completion without interruption"

**Internal monologue**: "Okay, Auto runs by itself. Interactive probably lets me chat. I'll leave it on Auto."

### First Obstacle

Clicks Run.

A terminal window opens. Text starts streaming: `→ Read: README.md`, `→ Read: STYLE.md`, then paragraphs of output.

**Internal monologue**: "What's this black window? Is this the AI? Why didn't it just show me the result in the app?"

They don't know how to interact with the terminal. The AI is writing to a file but they don't see that.

**Confusion**: The output in terminal is mostly code-related operations (reading files, writing code) even though they asked for a document. The interface doesn't explain what's happening.

**Internal monologue**: "I just wanted a document. Why is it reading all these code files? Did I do something wrong?"

### Recovery

They wait. The task completes. A new item "gentle-falcon" appears in the sidebar.

**Internal monologue**: "'gentle-falcon'? What's that? Is that where my document is?"

They don't know how to access it. They try:
1. Clicking on "gentle-falcon" in the sidebar → selects it but doesn't open anything visible
2. Right-clicking → sees options including "Reveal in Finder"
3. Clicks "Reveal in Finder" → Finder opens to a folder

**Internal monologue**: "Oh, there's a folder. Let me look inside..."

They navigate the folder. Eventually find a new markdown file with their requirements doc.

**Assessment**: The task worked but the path to results was obscure. They expected ChatGPT-style inline results. Instead they got a terminal process that wrote to a hidden folder.

### Verdict

**Would they come back tomorrow?** Unlikely. The tool is too developer-centric:
- Terminal output is intimidating
- Results are buried in folder structures
- No inline preview of what was created
- The "worktree" concept doesn't map to their mental model

They might try again if a developer colleague walks them through it. Otherwise, they'll stick to ChatGPT/Claude web for their needs.

### Pain Points

- [ ] Terminal output intimidating for non-engineers—no explanation of what's happening
- [ ] Results not shown in app—have to navigate to folder to find them
- [ ] "Worktree" concept unclear—what's a branch? what's isolation mean for docs?
- [ ] Task selector options seem code-focused ("implement", "review")—where's "write doc"?
- [ ] No preview of created content before closing
- [ ] Can't iterate or refine in place—would need to run again
- [ ] Auto-generated name "gentle-falcon" is cute but doesn't help identify content
- [ ] Output panel shows streaming text but doesn't explain what AI is doing

---

## Summary

### Top 3 Issues Across All Profiles

1. **Context Opacity**
   - Users can't see what context is being assembled
   - Token count is a single number with no breakdown
   - No CLI `-c` equivalent to inspect the full prompt
   - Power User: "I'm flying blind"
   - New Developer: Doesn't even know context is a concept
   - **Impact**: Critical for Power User, moderate for others

2. **Mental Model Gap**
   - Maestro is a launcher that opens terminal sessions
   - Results appear in terminal and worktree folders, not in-app
   - Designer/PM expected ChatGPT-style inline responses
   - New Developer surprised by terminal transition
   - **Impact**: Critical for Designer/PM, significant for New Developer

3. **Dual Input Confusion**
   - Task selector and typed prompt compete
   - Prompt picker appears while typing, interrupting flow
   - Users don't know if they need to select a task first
   - Power User: "Two entry points that conflict"
   - New Developer: Unsure which input matters
   - **Impact**: High for Power User, moderate for New Developer

### Pain Points by Profile

| Issue | New Developer | Power User | Designer/PM |
|-------|--------------|------------|-------------|
| Context opacity | Doesn't know to check | Critical blocker | N/A |
| Dual input confusion | Moderate confusion | Significant friction | Mild confusion |
| Terminal output | Unexpected but okay | Expected but redundant | Intimidating |
| Results visibility | Wants summary | Wants context preview | Wants inline results |
| Running state invisible | Minor | Significant | N/A |
| Worktree concept | Understands basics | Already knows | Doesn't map |
| Task selector purpose | Unclear | Clear but redundant | Seems code-focused |

### What Each Profile Values

| Profile | Primary Value | Would Use For |
|---------|--------------|---------------|
| New Developer | "AI writes code in safe folder" | Learning, small features |
| Power User | Worktree overview, PR actions | Management, not launching |
| Designer/PM | (Low value currently) | Would use for docs if inline |

### Recommendations

**For New Developer**:
1. Show toast when worktree is auto-created: "Created 'curious-dolphin' for this task"
2. Add "View Results" button after task completes—opens diff or folder
3. Explain token count on hover: "How much context the AI will see"

**For Power User**:
1. Add context preview panel—show exactly what will be sent
2. Remove dual input—either Task selector OR slash commands, not both
3. Show running state on worktree rows—spinner during execution
4. Add Cmd+K command palette for keyboard-first navigation

**For Designer/PM**:
1. Consider in-app result preview for document-focused tasks
2. Add task presets for non-code use cases ("write spec", "review doc")
3. Show what AI created before closing—inline preview
4. Reduce terminal exposure—or explain what's happening in plain language

### Context Preview Priority

The context preview panel (currently in `.design/ux-agent.md`) addresses the #1 issue across all profiles. Implementation should:

1. Make token count clickable → expands preview panel
2. Show breakdown by section: Docs (1.2k), Files (5.5k), Diff (0.3k)
3. List individual files in each section
4. Allow removal via ✕ button
5. Include Copy button (CLI `-c` parity)
6. Update in real-time when toggles change

This directly addresses Power User's "flying blind" complaint and helps New Developer understand what context means.
