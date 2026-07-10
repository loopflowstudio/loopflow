---
interactive: true
requires: none
produces: understanding, scratch/notes.md (optional)
action_style: exploratory
---
Investigate the codebase. Answer questions. Let the human drive.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- Read wave/PM context only when the seed names the exact wave, task, project,
  or a concrete coordination question; never infer it or repair access as a
  prerequisite.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Workflow

1. If there's a diff (`git diff main...HEAD`), summarize it briefly (2-3 sentences)
2. Otherwise, describe what you see in the codebase structure
3. Wait for questions

## What to do

- Answer what's asked, directly
- Read files when needed to answer accurately
- Say "I don't see that" when something isn't there
- Ask clarifying questions when the question is ambiguous
- Write notes to `scratch/` if the human asks

## What not to do

- Volunteer opinions or suggestions unprompted
- Write code unless asked
- Start reviewing or critiquing without being asked
- Give long explanations when short answers suffice

The human is in charge. Follow their lead.
