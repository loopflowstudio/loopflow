---
voice: architect
---
> **Screenshots**: If running standalone, capture with Cmd+Shift+S first.
> In the `ux` pipeline, uses screenshots from the ux-research step.
>
> **Build from this branch**: Run `cd swift && ./dev run` to build and launch
> Concerto from the current branch. Don't use the installed app—it won't have your changes.

Implement high-priority UX improvements based on research and gap analysis.

Read `swift/DESIGN.md` for the design philosophy guiding these changes.

## Input

Read the research artifacts:
- `.design/ux-research.md` - User pain points by profile
- `.design/ux-gaps.md` - Gaps vs Figma/Cursor/Notion

## Focus Areas

Prioritize improvements in this order:

### 1. First-Run Experience
- Setup clarity and progress
- What users see before any action
- Empty states that guide

### 2. Prompt Input Flow
- Input affordances (what can I type?)
- Feedback (what will happen?)
- Recovery (how do I fix mistakes?)

### 3. Defaults and Configuration
- Sensible defaults that work out of box
- Progressive disclosure of advanced options
- Reduce decisions for new users

### 4. Error & Empty States
- **Affordances over status**: "Connect lfd" not "lfd not connected"
- Every error has a recovery action button
- Empty states guide to the next action
- Graceful degradation with clear path forward

## Constraints

- **Small changes**: Each fix should be one commit's worth
- **No redesigns**: Improve what exists, don't rebuild
- **Test in Concerto**: Build and verify each change works
- **macOS conventions**: Follow platform patterns

## Process

For each improvement:

1. Identify the specific file(s) to change
2. Make the minimal edit
3. Build and run: `cd swift && ./dev run`
4. Describe the before/after behavior
5. Commit with message: `concerto: [area] short description`

## Output

After making changes, update `.design/ux-fixes.md`:

```markdown
# UX Fixes Applied

## [Area]: [Short description]
**Problem**: From research/gaps
**Change**: What was modified
**Files**: List of files
**Commit**: Hash

## ...

## Remaining
Issues identified but not yet fixed:
- [ ] Issue
- [ ] ...
```

Make real code changes. Build and verify.
