---
voice: customer
---
> **Before running**: Open Concerto, navigate to each screen you want reviewed,
> and press Cmd+Shift+S to capture. Screenshots save to `/tmp/concerto-<timestamp>.png`.
>
> **Build from this branch**: Run `cd swift && ./dev run` to build and launch
> Concerto from the current branch. Don't use the installed app—it won't have your changes.

Conduct user research by simulating three user profiles experiencing Concerto for the first time.

## Part 1: Screenshot Capture

Use Concerto's debug capture (Debug menu or keyboard shortcut) to generate screenshots of key states:
- Welcome/setup screen
- Empty repo state (no worktrees)
- Prompt input with various toggle states
- Running state
- Error states

Save to `.design/screenshots/` with descriptive names.

## Part 2: Visual Audit

For each screenshot, review:
- Alignment and spacing inconsistencies
- Typography hierarchy problems
- Color contrast and accessibility
- Visual clutter or unclear affordances
- macOS platform conventions

## Part 3: User Profile Simulation

Walk through as three personas:

### 1. New Developer
- Just heard about "AI coding assistants"
- Has never used Claude Code, Cursor, or Copilot
- Comfortable with git but not power user
- Wants to understand: "What can this do for me?"

### 2. Claude Code Power User
- Uses `claude` CLI daily
- Knows prompts, context management, worktrees
- Trying Concerto to see if GUI is faster
- Expects: feature parity with CLI, plus visual benefits

### 3. Designer or PM
- Non-engineer, curious about AI assistance
- Might use for docs, specs, or light scripting
- Low tolerance for jargon or complexity
- Needs: clear affordances, forgiving errors

For each profile, narrate their experience:
1. **First impression** (0-5 seconds): What do they see? What do they think this does?
2. **First action**: What do they try to do? Can they figure out how?
3. **First obstacle**: Where do they get stuck or confused?
4. **Recovery**: Can they recover? Do they give up?
5. **Verdict**: Would they come back tomorrow?

Be specific. Quote what they see. Note exact UI elements that confuse or delight.

## Output

Write findings to `.design/ux-research.md`:

```markdown
# UX Research

## Screenshots Captured
- [list of screenshot paths]

## Visual Issues
- [ ] Issue and location
- [ ] ...

## User Profile Findings

### New Developer
**First impression**: ...
**First action**: ...
**First obstacle**: ...
**Recovery**: ...
**Verdict**: ...

#### Pain Points
- [ ] Specific issue and location

### Claude Code Power User
...

### Designer/PM
...

## Top 5 Priority Issues
1. ...
2. ...
3. ...
4. ...
5. ...
```

Do not write code. Research only.
