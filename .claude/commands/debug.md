Debug an error using the stacktrace or error message from clipboard.

Run with `-v` to include clipboard content:
```bash
lf debug -v
```

## Workflow

1. Parse the error/stacktrace from the clipboard content
2. Identify the file and line number where the error originated
3. Run `git diff main...HEAD -- <file>` to see if this file was changed on this branch
4. Read the relevant files to understand context
5. Fix the bug
6. Run `uv run pytest tests/` to verify the fix doesn't break anything

## Debugging strategy

**Follow the stack trace.** The deepest frame in your code (not library code) is usually where the problem originates. Start there.

**Check recent changes.** If the error is new, run `git diff` to see what changed. The bug is likely in the delta.

**Reproduce first.** Before fixing, understand how to trigger the error. A fix you can't verify isn't a fix.

## Common patterns in this codebase

**Import errors.** Imports are at top of file. Check if a new import was added but not the dependency.

**Path errors.** Loopflow uses `Path` objects. Check if something expects a string or vice versa.

**Subprocess failures.** Many operations shell out to git, claude, codex. Check if the subprocess command is correct and the tool is available.

**Config errors.** `.lf/config.yaml` parsing uses Pydantic. Check if required fields are missing or types are wrong.

## Output

Fix the bug directly. If the cause isn't obvious from the fix, add a brief inline comment.

If you can't determine the cause, describe what you learned and what additional context is needed.

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.

