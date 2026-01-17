# UX Gap Analysis

*Artist voice: Question the constraints. What's the wildly different approach nobody's considering?*

---

## Visual Audit

### The Screenshot Tells a Story

The captured screenshot reveals a fundamental architectural problem, not just visual issues.

What we see:
1. A macOS permission dialog blocking the interface
2. The dialog identifies the app as **"MaestroU0.2026.01.14.08.16.sta-9kc_042"**—gibberish
3. Maestro running alongside Cursor
4. Claude Code streaming in Cursor's terminal, not in Maestro

The first moment a user encounters Maestro, they see a system dialog that looks like malware asking for permissions. Behind it, they discover that the "output" lives somewhere else entirely.

**This isn't a visual polish issue. It's an identity crisis.**

### Visual Issues Worth Noting

- [ ] **Permission dialog app name** — Auto-generated bundle identifier shown to users. Trust dies here.
- [ ] **Split attention** — Output in Terminal, controls in Maestro. The workflow is fragmented.
- [ ] **Typography inconsistency** — `.caption` vs `.caption2` mixed without system. Pick one.
- [ ] **Context chips overflow** — Five chips can exceed viewport with no constraint.
- [ ] **Token count orphaned** — Neither prominent nor hidden. Floating in uncertainty.
- [ ] **Disabled states at 0.5 opacity** — Potentially fails WCAG contrast requirements.
- [ ] **Results header density** — Five controls fighting for one row.

---

## Welcome/Setup

### Current State

Icon, "Loopflow Maestro", "Tell it what to build. It writes the code.", recent repos, "Open Folder" button.

### The Gap

It's a file picker with marketing copy. Users must already intend to "open a repo" before seeing any evidence the tool works.

### Why does it have to be this way?

**What if the welcome screen WAS the product running?**

Imagine: You launch Maestro. A bundled sample project is already loaded. A task is running. You watch code appear, tests run, files change. The welcome screen isn't a gate—it's proof.

"Want to try with your own code? [Open a repo]" appears after you've seen magic happen.

Or: **Clipboard mode**. The welcome screen has a paste zone. Drop code, describe a change, watch results. No repo. No git. No commitment. Value delivered before commitment requested.

Or: **Video proof**. A 10-second loop showing the workflow, muted, autoplaying. "Tell it what to build" becomes undeniable when you watch it happen.

### Patterns to adopt

1. **Demo-first** (Figma) — You're drawing in 10 seconds
2. **Zero-commitment entry** (Paper) — Creativity shouldn't require setup
3. **Show, don't tell** — Video > tagline

---

## Prompt Input

### Current State

Task dropdown with typeahead. Large text field. Mode picker (Auto/Interactive). Run button. "implement" pre-selected.

### The Gap

Four conceptual steps: select task → configure mode → type → run.

The task dropdown is an admission: "We don't know what you want." The mode picker is another: "We can't decide which is better."

### Why does it have to be this way?

**What if there was just a text field?**

```
┌──────────────────────────────────────────────────────┐
│ add authentication to the login page                 │
│                                    implement ⌘↵     │
└──────────────────────────────────────────────────────┘
```

- **No task selector**. Type "add auth" → AI infers `implement`. Type "review the changes" → infers `review`. Ghost text shows inference. Tab confirms. Wrong? Type `/design` explicitly.
- **No mode picker**. Default: auto. Task frontmatter controls exceptions. The 95% who never change it never see it exists.
- **No Run button taking space**. Cmd+Enter runs. The button is training wheels.

### Even wilder

What if the prompt input was ALSO a command palette? Start typing, and suggestions include:
- Tasks that match ("implement: add auth...")
- Recent prompts
- Slash commands ("/design", "/review")
- Files ("@src/auth.ts")

One unified input that does everything.

### Patterns to adopt

1. **Intent inference** — Confident tools infer, uncertain tools ask
2. **Ghost completion** (Copilot) — Show inference, Tab confirms
3. **Slash commands** (Notion) — Explicit override, not default
4. **Unified input** (Raycast) — One box for everything

---

## Context Controls

### Current State

Five toggle chips: Docs, Files, Diff, Clipboard, Summaries. Drag-and-drop attachments. Token count breakdown.

### The Gap

Five decisions before first run. "What are tokens? Why 14.2k? Is that good?"

The toggles serve power users. Power users are 10%. Default UX optimizes for the wrong 10%.

### Why does it have to be this way?

**What if context was invisible?**

- **Smart defaults per task**: `design` → docs. `review` → diff. `implement` → design doc + touched files. Users never configure.
- **@ mentions for override**: Type `@src/auth.ts` in the prompt. Typing beats toggling.
- **Token budget as progress bar**: Thin bar under input. Green → yellow → red. Numbers on hover only.
- **No chips visible by default**: Context is there. It's working. You don't need to see it unless you ask.

Power users expand "Context" to see what's included. Everyone else never knows this section exists.

### Patterns to adopt

1. **Automatic context** (Cursor) — It just knows
2. **@ mentions** (Cursor) — Surgical override
3. **Visual budgets** — Progress bars beat numbers
4. **Hide until asked** — Real progressive disclosure

---

## Worktree Sidebar

### Current State

"Workspaces" header. Branch names. Commit counts. Stage badges. Hover actions.

### The Gap

The sidebar shows git primitives. Users don't care about git. They care about work:
- "What am I building?"
- "What's running?"
- "What needs attention?"

### Why does it have to be this way?

**What if the sidebar showed intent, not implementation?**

```
IN PROGRESS
  "add authentication"      implementing... 2:34 ●
  "api refactor"            needs review

READY
  "fix typos"               1 commit, clean

BLOCKED
  "new UI"                  conflicts with main
```

The initial prompt becomes the workspace name. Branch names appear on hover for those who care. Work state groups items naturally.

### Even wilder

What if there was no sidebar?

Maestro could be **single-pane**. The current workspace is "wherever you last ran a task." Switch workspaces via Cmd+K. The sidebar appears when you invoke it, not permanently.

Most sessions involve one workspace. Why show a list of N workspaces when you're using 1?

### Patterns to adopt

1. **Work state grouping** (Linear) — In Progress / Ready / Blocked
2. **Intent as identity** — Prompt text > branch name
3. **Minimize chrome** (Ive) — Maybe no sidebar at all
4. **Cmd+K for switching** (Linear) — Search > browse

---

## Running State

### Current State

Task launches external terminal. Sidebar shows pulsing dot. ResultsPanel shows "Running..." with spinner. Embedded terminal exists but toggle is buried.

### The Gap

Launch in Maestro → watch in Terminal → return to Maestro. Three context switches per task.

SwiftTerm is implemented. It works. But the toggle is in "More options" → scroll → find it. Users who don't discover it experience the broken flow.

### Why does it have to be this way?

**What if there was no external terminal for auto mode?**

Make embedded terminal the default. No toggle. No choice. Output appears in Maestro. Period.

But go further: **What if raw terminal wasn't the interface at all?**

```
implement: add authentication            2:34 ●

Reading codebase...
  Found auth patterns in src/middleware

Planning changes...
  ✓ Create src/auth.py
  ✓ Update src/routes.py
  → Writing tests...

[Show Terminal] [Stop]
```

Structured phases. Progress indication. Current file. Raw terminal as escape hatch for debugging.

### Even wilder

What if running tasks were **invisible by default**?

You type, hit Cmd+Enter, and the prompt clears. In the corner: "Implementing... ●". That's it.

System notification when done: "auth-feature complete. 3 files changed." Click → see results.

Background by default. Foreground on demand. The tool gets out of the way.

### Patterns to adopt

1. **No choice** — Embedded terminal is the only option for auto mode
2. **Structured progress** — Phases beat firehose
3. **Background by default** — Notification on completion
4. **Minimal chrome** — The less visible while running, the better

---

## Errors/Empty States

### Current State

Worktree empty state is decent. Other areas: blank or generic SwiftUI alerts.

### The Gap

Voids are missed opportunities. Errors are dead ends.

### Why does it have to be this way?

**Every empty state is a chance to teach.**

Empty prompt area:
> What should AI build for you?
> [add auth] [fix bugs] [write tests] ← clickable

Empty results:
> Your code is waiting. Run a task above.

On main branch:
> You're on main. Create a workspace so AI changes don't affect your code.
> [Create workspace]

**Every error is a conversation.**

Worktree exists:
> 'auth' already exists.
> [Open auth] [Create auth-v2]

Can't find Warp:
> Warp not found.
> [Install Warp] [Use Terminal.app] [Use embedded terminal]

### Patterns to adopt

1. **Clickable examples** — Empty states that run on click
2. **Actionable errors** — Buttons, not just text
3. **Conversation** — Errors are dialogue, not walls

---

## Summary: Priority Gaps

| Gap | Impact | Design Principle Violated |
|-----|--------|--------------------------|
| **Output in external terminal** | Critical | Immediate Connection (Bret Victor) |
| **No demo/onboarding** | High | Remove Barriers (fast.ai) |
| **Context requires expertise** | High | Progressive Disclosure (Notion) |
| **Task selector over-engineered** | Medium | Opinionated Defaults (Linear) |
| **Permission dialog gibberish** | Medium | Craft Signals Care (Ive) |
| **Mode picker meaningless** | Medium | Design Should Disappear (Ive) |

---

## Patterns to Steal

### From Cursor
1. **Inline streaming** — Output where you launched
2. **@ mentions** — `@file.ts` beats toggles
3. **Automatic context** — Per-task defaults

### From Notion
4. **Slash commands** — `/design` as override
5. **Empty states as opportunities** — Clickable examples
6. **Progressive disclosure** — Simple surface, depth available

### From Linear
7. **Cmd+K everywhere** — Searchable actions
8. **Work state grouping** — In Progress / Ready / Blocked
9. **Opinionated defaults** — One way, done well

### From Figma
10. **Demo before commitment** — Show value first
11. **Performance obsession** — Sub-100ms
12. **Remove friction before features**

---

## Wild Ideas

### Why is Maestro an app?

What if it wasn't?

- **Raycast extension**: Cmd+Space → "lf implement: add auth" → done
- **Menu bar**: Click → running tasks. Right-click → quick launch
- **VS Code sidebar**: Panel in the IDE you already use
- **Notifications only**: Tasks run silently, notify on complete

The app exists for visual orchestration. Maybe visual orchestration is a panel, not a window.

### Why select a task at all?

What if intent was inferred?

- "add authentication" → implement
- "review the changes" → review
- "what does this do" → explain
- "fix the bug" → debug

Ghost text shows inference. Tab confirms. `/task` for explicit override.

### Why show branch names?

What if it showed prompts?

```
IN FLIGHT
  "add authentication"      implementing...
  "fix login bug"           ready for review

DONE TODAY
  "update readme"           merged 2h ago
```

Users remember what they asked for, not the branch.

### Why require a repo?

What if you didn't?

- **Clipboard mode**: Paste code, describe change, see result
- **Template starters**: "New CLI tool" → creates repo, runs first task
- **URL mode**: Point at a GitHub repo without cloning

The repo requirement assumes git expertise. Not everyone has it.

---

## The Core Insight

Maestro optimizes for users who understand:
- Git and worktrees
- LLM context and tokens
- Loopflow's task model
- Terminal workflows

What if it optimized for someone who knows **none** of that?

They want:
1. Tell computer what to build
2. Watch it happen
3. Review result
4. Keep or reject

That's the 80% case.

**Ideal Maestro**:

```
┌──────────────────────────────────────────────────────┐
│ add authentication                           ⌘↵     │
└──────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────┐
│ Writing code...                              2:34 ● │
│                                                      │
│ ✓ Created src/auth.py                               │
│ ✓ Updated src/routes.py                             │
│ → Writing tests...                                   │
└──────────────────────────────────────────────────────┘

              [Keep changes]  [Try again]
```

One text field. Progress. Two buttons.

Everything else exists—but hidden.

**The goal: make the 80% case require zero decisions.**

Linear refused Jira. fast.ai embedded best practices. Paper gave five brushes. Figma removed friction first.

Maestro should:
1. Default to implement ✓ (done)
2. Auto-select context per task ✗ (gap)
3. Stream results in-app ✗ (toggle buried)
4. Hide advanced options ✗ (partial)
5. Infer task from prompt ✗ (gap)
6. Use work language, not git ✗ (partial)

**The tool should work without configuration. Configuration is depth, not prerequisite.**
