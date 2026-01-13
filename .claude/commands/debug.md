Debug an error using the stacktrace or error message provided via clipboard.

Run this task with `-v` to include the error from your clipboard:

```bash
lf debug -v
```

## Process

1. Read the error/stacktrace from the clipboard content
2. Identify the relevant files from the stacktrace or error message
3. Read those files to understand the context
4. Diagnose the root cause
5. Fix the issue

## What to look for

**Stack traces.** Follow the call stack from the error back to the root cause. The deepest frame in your code is usually where the problem originates.

**Error messages.** Parse the error type and message for clues. Import errors, type errors, and assertion failures each suggest different fixes.

**Context.** Check recent changes with `git diff` if the error is new. Look at surrounding code to understand the intended behavior.

## Output

Fix the bug directly. Explain what went wrong in a brief comment if the cause isn't obvious from the fix itself.

If you can't determine the cause from the available information, say what additional context you need.

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.

