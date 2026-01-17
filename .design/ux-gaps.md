# UX Gap Analysis

## Visual Issues

### From Screenshot Audit

The captured screenshot reveals Maestro running alongside Cursor with a macOS "Screen & System Audio Recording" permission dialog blocking the interface. Key observations:

- [ ] **Permission dialog app identifier** — Shows "MaestroU0.2026.01.14.08.16.sta-9kc_042" which looks like gibberish to users. This erodes trust immediately—the first system interaction resembles malware.
- [ ] **Output streams in external terminal** — The visible IDE pane shows Claude Code streaming output in Cursor's integrated terminal, not in Maestro. Users launch from Maestro but watch results elsewhere.
- [ ] **Window management burden** — Screenshot shows multiple overlapping windows (Maestro, permission dialog, IDE). The workflow requires juggling windows rather than focusing on work.

### Alignment and Spacing
- [ ] **Context chip overflow** (`PromptLauncher.swift:947-1007`): Five chips have no max-width constraint. With many attachments, they extend beyond viewport.
- [ ] **Empty state vertical drift** (`WorktreeSidebar.swift:168-196`): Content floats low in tall windows due to `maxHeight: .infinity` instead of optical centering.
- [ ] **Results panel header density** (`ResultsPanel.swift:64-120`): Five controls compete in one row. Hierarchy is flat—status, text, duration, toggle, clear, expand all at equal visual weight.
- [ ] **Options section lacks grouping** (`PromptLauncher.swift:60-66`): Model selector, voice selector, context bar, command preview run together without visual separation or hierarchy.

### Typography Hierarchy
- [ ] **Mixed caption sizes**: Inconsistent use of `.caption` vs `.caption2` throughout. No clear typographic scale—some secondary text is `.caption2`, some is `.caption` with `.tertiary` color.
- [ ] **Token count placement** (`PromptLauncher.swift:740-756`): Embedded in crowded row with mode picker and options button. Neither prominent enough to matter nor hidden enough to ignore.

### Color and Contrast
- [ ] **Disabled state opacity** (`PromptLauncher.swift:886`): Context sections at `opacity(0.5)` may fail WCAG AA 4.5:1 contrast requirements.
- [ ] **Running state indicator** (`WorktreeRow.swift:659-664`): Pulsing blue dot relies on animation alone—accessibility concern for users with reduced motion enabled.
- [ ] **Selected worktree** (`WorktreeRow.swift:527-530`): Blue accent at 15% opacity is too subtle for primary selection state. Compare to Notion's bolder selection highlight.

### Visual Clutter
- [ ] **Five context chips always visible** (`PromptLauncher.swift:947-958`): Docs, Files, Diff, Clipboard, Summaries shown immediately. Overwhelming cognitive load for new users who don't understand what these mean.
- [ ] **Four hover actions on worktrees** (`WorktreeRow.swift:593-647`): Diff, PR, Terminal, IDE all appear on hover with similar small icons—hard to distinguish at a glance.

### macOS Platform Conventions
- [ ] **Sheet sizing** (`NewWorktreeSheet.swift:756`): Fixed 320pt width feels cramped on larger displays; should scale or have sensible minimum.
- [ ] **No File > Open Recent**: Recent repos only accessible from welcome window. Working users can't quickly switch repos without returning to welcome.

---

## Welcome/Setup

**Current**: `WelcomeWindow.swift` shows icon (`wand.and.sparkles`), "Loopflow Maestro" title, "Tell it what to build. It writes the code." subtitle, recent repos list, and "Open Folder" button. This is improved from the previous "AI coding assistant" abstraction.

**Inspiration—Figma**: You're drawing in under 10 seconds. No "here's what this is" preamble. The interface teaches through interaction.

**Inspiration—Notion**: Empty states offer templates demonstrating value. "Press / for commands" teaches the core interaction. Productive before understanding.

**Gap**: The welcome screen is a repo picker, not an introduction to what happens after picking one. Users must already understand they want to "open a repo" before seeing any value demonstration.

**Why does it have to be this way?**

What if the welcome screen was proof?

1. **Playable demo**: Bundled sample project users can run tasks on. See the full loop—prompt → agent runs → code changes → diff review—before committing their own code.

2. **Auto-playing video**: 15-second loop showing the workflow. No sound, just visual proof. "Tell it what to build. It writes the code." becomes undeniable when you watch it happen.

3. **Clipboard mode**: Paste code, describe a change, see results. Zero commitment, immediate value.

The current design assumes intent. What if we created intent by showing undeniable proof?

**Patterns to adopt**:
1. **Demo-first**: Let users experience before committing
2. **Visual proof**: Show, don't claim. Video > tagline.
3. **Graceful degradation**: Work without a repo for simple tasks

---

## Prompt Input

**Current**: Task typeahead selector (with "implement" pre-selected), large text area with placeholder "What should the AI build?" and example text, mode picker (Auto/Interactive), Run button. Context chips below. Command preview behind "More options" toggle.

**Inspiration—Cursor**: Chat is immediate. Context is automatic. `@` mentions for surgical override. The prompt adapts to what you're doing—no mode switches.

**Inspiration—Notion**: `/` commands discoverable as you type. Feels like writing, not operating software.

**Gap**: The task selector, while improved with typeahead, still requires knowing tasks exist and which one you want. Mode picker ("Auto"/"Interactive") remains meaningless to newcomers. The interaction is: select task → configure mode → type → run. Four conceptual steps.

**Why does it have to be this way?**

What if you just typed?

1. **Intent inference**: "add auth to the login page" → infers "implement". "review the changes" → infers "review". Suggestion appears, Tab to accept.

2. **Slash commands as escape hatch**: `/design` explicit override. But default is inference, not selection.

3. **No visible mode picker**: Default to auto. Task frontmatter controls mode. Most users never need to know modes exist.

4. **Context automatic per task**: `design` → include docs. `review` → include diff. `implement` → include design doc. No chips to understand.

The dropdown exists because we're uncertain about intent. A confident tool infers intent and lets users correct it.

**Patterns to adopt**:
1. **Ghost completion**: Show inferred task as user types
2. **Slash commands**: Explicit override, not default interaction
3. **Automatic context**: Smart defaults per task type
4. **Mode hidden**: Surface only when task requires it

---

## Context Controls

**Current**: Five toggle chips (Docs, Files, Diff, Clipboard, Summaries), drag-and-drop file attachment, expandable token count breakdown. No `@` mentions.

**Inspiration—Cursor**: Context is automatic. You don't toggle "include files"—it knows. Override is surgical: `@file.ts`.

**Inspiration—Figma**: Component panel shows what's relevant to selection. No searching required.

**Gap**: Five toggles demand understanding of token economics before first run. "What are tokens? Why 14.2k? Is that good?" Users make decisions about things they don't understand.

**Why does it have to be this way?**

What if context was invisible?

- **Smart defaults**: Each task knows what it needs. User never sees toggles unless they ask.
- **@ mentions for override**: `@src/auth.ts @README.md` in the prompt. Typing, not toggling.
- **Collapsed by default**: Show "14.2k tokens" only. Expand reveals breakdown. Most won't expand.
- **Learn once**: If user expands, remember preference. But start collapsed.

The toggles serve power users. But power users are 20%. The default experience should serve the 80% who want results, not configuration.

**Patterns to adopt**:
1. **Per-task defaults**: design → docs. review → diff. implement → design doc + files.
2. **@ mentions**: Inline file references
3. **Collapsed by default**: Token count summary, details on demand
4. **Remember preferences**: Once expanded, stay expanded

---

## Worktree Sidebar

**Current**: "Workspaces" header (improved from "BRANCHES"), list with branch names, commit counts, stage badges with icons (lightbulb/hammer/magnifyingglass/sparkles—accessibility fix applied), hover actions. Empty state explains workspaces clearly.

**Inspiration—Notion**: Page tree is effortlessly navigable. Icons show type at glance. Drag to reorder.

**Inspiration—Figma**: Layers panel mirrors canvas. Hover reveals actions without demanding attention.

**Gap**: Still organized around git primitives. Users see branch names, not work intent. Stage badges show last task—not what's running now or what needs attention next.

**Why does it have to be this way?**

Users care about work state, not git state:
- "What am I building?"
- "What's the agent doing?"
- "What needs my attention?"

**Wild idea**: Organize by work state.

```
IN PROGRESS
  auth-feature        implement running... 2:34
  refactor-api        needs review

READY TO MERGE
  fix-typo            1 commit, clean

BLOCKED
  new-ui              conflicts with main
```

Primary view: work-centric. Git details (branch, SHA) available on expand or hover.

**Patterns to adopt**:
1. **Work state grouping**: In Progress / Ready / Blocked
2. **Running tasks prominent**: Elapsed time, current status
3. **Next action visible**: "needs review" clickable
4. **Git details on demand**: Expand for technical info

---

## Running State

**Current**: Task launches external terminal. Sidebar shows pulsing blue dot. `ResultsPanel.swift` shows "Running {task}..." with spinner and elapsed time. Log output toggleable but defaults to result summary.

**Inspiration—Cursor**: Streams output inline. You see the agent working. Feels like watching someone type.

**Inspiration—Figma**: Presence indicators show system state at glance.

**Gap**: Results stream to external terminal, not Maestro. Users launch from Maestro → find Terminal → watch output → return to Maestro. Three context switches per task.

**Why does it have to be this way?**

External terminal exists because Claude Code is a CLI. But that's implementation, not user need. Users need to see their code change—they don't care how.

**Wild ideas**:

1. **In-app terminal via SwiftTerm**: Embed terminal in ResultsPanel. PTY attached, VT100 rendering, no context switch. SwiftTerm is production-ready (researched in `.design/terminal-embedding.md`).

2. **Progress phases instead of raw output**:
   ```
   implement: add authentication

   [====      ] Writing code...
               → src/auth.py
               → src/routes.py

   Step 2 of 4: Creating files
   ```
   High-level status, terminal available as escape hatch.

3. **Background with notification**: Task runs silently. System notification when done: "auth-feature complete. 5 files changed." Click → Maestro with results.

4. **Picture-in-picture**: Small floating terminal stays visible while working elsewhere.

SwiftTerm makes option 1 tractable. The UX cost of external terminal—context switches, lost windows, no completion notification—justifies the implementation effort.

**Patterns to adopt**:
1. **In-app streaming**: Embedded terminal via SwiftTerm
2. **Progress phases**: High-level status visible
3. **Completion notification**: System notification for background tasks
4. **Escape to full terminal**: Button for those who want raw output

---

## Errors/Empty States

**Current**: Worktree empty state is solid—icon, explanation, action button. Other areas lack empty states. Errors use generic SwiftUI alerts.

**Inspiration—Notion**: Empty pages feel like opportunities. "Press Enter to continue..."

**Inspiration—Figma**: Errors are specific and actionable. "Can't connect—use local fonts?"

**Gap**: Empty states elsewhere are missing or minimal. Errors say "An error occurred" with OK button.

**Why does it have to be this way?**

Every void is an opportunity for guidance:

- **Empty prompt area**: "Try: 'add user authentication' or 'fix the failing tests'" with clickable examples
- **Empty results panel**: "Your results will appear here after running a task."
- **On main branch**: "Create a workspace to let AI make changes safely. [Create]"
- **Worktree creation failed**: "Branch 'auth' exists. Try: auth-v2 [Create auth-v2]"

Errors should be conversations:
- "Couldn't start terminal—Warp not found. [Install Warp] [Use Terminal.app]"
- "Task failed. [View logs] [Try again] [Report issue]"

**Patterns to adopt**:
1. **Contextual empty states**: Different message based on app state
2. **Actionable errors**: Fix included, not just problem
3. **Recovery buttons**: Remediation in the error
4. **Clickable examples**: Empty states that teach by doing

---

## Summary: Priority Gaps

1. **Results stream to external terminal** — Impact: **Critical**
   - Every task requires context switch
   - New users don't know to check Terminal
   - No completion notification
   - SwiftTerm makes in-app solution viable

2. **No demo mode or onboarding** — Impact: **High**
   - Users must understand workflow before seeing value
   - "Tell it what to build" is promise, not proof
   - No way to try before committing a repo

3. **Context controls demand expertise** — Impact: **High**
   - Five toggles visible before first run
   - Token count unexplained
   - No smart defaults per task
   - No @ mentions for power users

4. **Mode picker meaningless** — Impact: **Medium**
   - "Auto"/"Interactive" unexplained
   - Users guess or ignore
   - Could be hidden entirely

5. **Permission dialog shows gibberish identifier** — Impact: **Medium**
   - First system interaction looks suspicious
   - Erodes trust before app is used

6. **Task selector still requires knowledge** — Impact: **Medium**
   - Improved with typeahead and default
   - But no intent inference from prompt
   - No slash commands

---

## Patterns to Steal

1. **From Cursor—Inline streaming output**
   - Apply to: ResultsPanel
   - Show output live via embedded terminal
   - Eliminates context switch

2. **From Cursor—@ mentions for context**
   - Apply to: Prompt input
   - `@file.ts` adds context inline
   - Faster than toggles, discoverable

3. **From Notion—Slash commands**
   - Apply to: Prompt input
   - `/design`, `/review` as explicit override
   - Typing beats dropdown

4. **From Notion—Empty states as opportunity**
   - Apply everywhere
   - Clickable examples that teach

5. **From Linear—Cmd+K command palette**
   - Apply globally
   - Every action searchable
   - Shortcuts visible

6. **From Figma—Demo before commitment**
   - Apply to welcome
   - Show value, then ask for repo

7. **From Linear—Opinionated defaults**
   - Apply to context
   - Smart defaults per task
   - Config as escape hatch

8. **From Stripe—Progressive disclosure**
   - Apply to context bar
   - Token count only visible
   - Expand for breakdown

---

## Wild Ideas (Artist Mode)

### Question: Why is Maestro a separate app?

What if it wasn't?

1. **Raycast extension**: Cmd+Space, "lf implement: add auth". Workspaces in sidebar. Full app optional.

2. **Menu bar agent**: Click → running tasks. Right-click → quick launch. Full window only when needed.

3. **VS Code extension**: Sidebar in the IDE you're already using. No window management.

4. **Pure CLI with notifications**: `lf implement` runs silently. System notification when done. No app.

The app exists for visual orchestration. Maybe visual orchestration is a sidebar, not a window.

### Question: Why select a task at all?

What if there was just a prompt?

- "add user authentication" → AI infers implement
- "review the changes" → AI infers review
- "what does this code do" → AI explains without changing

Tasks exist because loopflow is structured. Structure can be inferred, not demanded.

### Question: Why show branches?

What if the sidebar showed intentions?

```
IN FLIGHT
  "add authentication"      implementing...
  "fix login bug"           ready for review

COMPLETED TODAY
  "update readme"           merged 2h ago
```

Users care about what they asked for, not the git branch name.

### Question: Why stream raw terminal output?

What if output was structured?

```
implement: add authentication

Reading codebase...
  Found patterns in src/middleware

Planning changes...
  1. Create src/auth.py
  2. Update src/routes.py
  3. Add tests/test_auth.py

Writing code... ████████░░ 80%

[Show Terminal] [Stop]
```

Progress, not firehose. Terminal as escape hatch.

### Question: Why require a repo?

What if you didn't?

- **Clipboard mode**: Paste code, describe change, get result
- **Demo project**: Bundled sample for learning
- **Standalone prompts**: `lf --prompt review.md --input file.py`

The repo requirement assumes full git workflow. Not every use needs it.

---

## The Core Insight

Current design optimizes for power users who understand:
- Git and worktrees
- LLM context and tokens
- Loopflow's task model
- Terminal-based workflows

What if Maestro optimized for someone who knows none of that?

They want to:
1. Tell a computer what to build
2. Watch it build
3. Review the result
4. Keep or reject

That's the 80% case. Tasks, toggles, modes, branches—that's configuration for the 20%.

**Progressive disclosure: the 80% case shows 20% of the UI.**

Ideal Maestro:
1. Single text field: "What do you want?"
2. Run button
3. Inline streaming output
4. "Keep?" / "Try again?"

Everything else exists but is hidden until needed.

**The tool should work without configuration. Configuration is for depth, not prerequisite.**

Linear refused Jira complexity. fast.ai embedded best practices. Figma removed friction before features.

Maestro should:
1. Default to implement
2. Auto-select context per task
3. Stream results in-app
4. Hide advanced options until requested
5. Replace git jargon with work language

The goal: **make the 80% case require zero decisions.**
