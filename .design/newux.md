# Consolidate UX Pipeline

A clean 3-task pipeline for UX improvement: `ux-research` → `ux-gaps` → `ux-fix`.

## What to build

Merge `ux-review` into `ux-research` so research generates and reviews screenshots inline, then delete `ux-review`. The result is a streamlined pipeline where each task has a clear input/output handoff.

## Pipeline Design

```
ux-research          ux-gaps              ux-fix
┌────────────────┐   ┌────────────────┐   ┌────────────────┐
│ Generate shots │   │ Compare to     │   │ Implement      │
│ Review UI      │ → │ best-in-class  │ → │ priority fixes │
│ Simulate users │   │ tools          │   │                │
└────────────────┘   └────────────────┘   └────────────────┘
     ↓                    ↓                    ↓
.design/             .design/             .design/
ux-research.md       ux-gaps.md           ux-fixes.md
screenshots/
```

## Data structures

No new code. This is prompt file restructuring.

Files to modify:
```
.claude/commands/ux-research.lf   # Merge in screenshot review
.claude/commands/ux-review.lf     # Delete
.claude/commands/ux-gaps.lf       # Adjust to depend on ux-research.md
.claude/commands/ux-fix.lf        # No change needed
```

## Key changes

### ux-research.lf

Merge screenshot review into user research. New structure:

```markdown
---
context:
  - .design/screenshots/
  - Maestro/Maestro/Views/
voice: customer
---
# UX Research

## Part 1: Screenshot Capture

Use Maestro's debug capture (⌘⇧C or menu) to generate screenshots of key states:
- Welcome/setup screen
- Empty repo state
- Prompt input with various toggle states
- Running state
- Error states

Save to .design/screenshots/ with descriptive names.

## Part 2: Visual Review

For each screenshot:
- Alignment and spacing issues
- Typography hierarchy
- Color contrast and accessibility
- Unclear affordances
- macOS convention violations

## Part 3: User Profile Simulation

Walk through as three personas:
1. Curious Beginner - "What can I even ask?"
2. CLI Convert - expects feature parity
3. Prompt Explorer - knows ChatGPT, not worktrees

For each: first impression, first action, first obstacle, recovery, verdict.

## Output

Write to .design/ux-research.md:
- Screenshots captured (paths)
- Visual issues found
- Per-profile friction points
- Top 5 priority issues
```

### ux-gaps.lf

Adjust context to read from ux-research.md:

```markdown
---
context:
  - .design/ux-research.md    # Input from previous step
  - .design/screenshots/
  - Maestro/Maestro/Views/
voice: artist
---
```

Body stays largely the same—compare against Figma/Cursor/Notion, apply design principles, identify gaps.

### ux-review.lf

Delete. Its content is now in ux-research.

## Constraints

- **Screenshot capture is manual**: The agent prompts the user to use ⌘⇧C in Maestro. It can't programmatically trigger captures.
- **Screenshots must exist**: ux-gaps and ux-fix expect .design/screenshots/ to be populated by ux-research.
- **Clean handoffs**: Each task reads the previous task's .design/*.md output.

## Done when

1. `ux-review.lf` is deleted
2. `ux-research.lf` includes screenshot generation + visual review sections
3. `ux-gaps.lf` context includes `.design/ux-research.md`
4. Running `lf ux-research` followed by `lf ux-gaps` followed by `lf ux-fix` works as a coherent pipeline
5. No references to `ux-review` remain
