---
requires: code on branch
produces: scratch/qa-findings.md
default_agent: codex
action_style: procedural
---
Thorough quality assessment of the current branch state.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- If a `wave/<name>/` directory matches this work, skim its roadmap and items.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Goal

Find every issue that would block deploy or embarrass the team. One deep pass beats five shallow checks.

## Workflow

1. Read the diff against main. Understand what changed and why.
2. Run the full test suite. Note failures, flaky tests, and missing coverage.
3. Read scratch/ for design intent. Check whether the implementation matches.
4. Walk through the code changes looking for:
   - Bugs: logic errors, off-by-ones, race conditions, null handling
   - Regressions: existing behavior broken by the changes
   - Security: injection, auth bypass, data exposure
   - Edge cases: empty inputs, large inputs, concurrent access
5. If the codebase has a UI, verify the user-facing changes work correctly.

## Output

Write findings to `scratch/qa-findings.md`:

```markdown
## Blocking Issues

Issues that must be fixed before deploy.

- [description of issue, file:line, severity]

## Polish Items

Non-blocking improvements to track for later.

- [description, file:line]

## Test Results

[summary of test run: passed/failed/skipped counts, notable failures]
```

Be specific. File paths, line numbers, reproduction steps. Vague findings waste the fix cycle.
