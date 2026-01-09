Review the diff on the current branch against `main` and fix any issues found.

## Focus areas

- Bugs and correctness issues
- Code clarity and maintainability
- Adherence to STYLE.md conventions
- Test coverage for new functionality

## Process

1. Read STYLE.md if present
2. Examine the diff between main and current branch
3. Identify issues and fix them directly
4. Run tests to verify fixes
5. Update the design doc (see below)

Fix issues as you find them. Don't just report problems—solve them.

## Design doc

If a design doc exists (`<branch>.md` at repo root), transform it into a human review guide:

- Remove implementation details that are now in code
- Add a "Review checklist" section with specific things for humans to verify
- Note any tradeoffs made, areas of uncertainty, or things that need manual testing
- Keep it concise—focus on what humans need to check, not what the code does

The design doc stays until the PR lands (`lf pr land` removes it).
