---
requires: error message or stacktrace (via clipboard -c)
produces: fixed code
diff_files: false
---
Debug an error using the stacktrace or error message from clipboard.

If clipboard is empty or no -c flag, ask what error to debug.

## What makes a good fix

**Unblock first.** Ask: what would it take to unblock the person who wants this debugged? Sometimes that's a quick workaround or explanation before a deeper fix. Get them moving, then address the root cause.

**Loop until the root issue is addressed.** Don't just take the next step and stop. Fix, verify, see what happens. If a new error surfaces, keep going. The job is done when the original workflow succeeds.

**Minimal and targeted.** Fix the bug, not the neighborhood. Don't refactor, don't "improve while you're here."

**Grease the wheels.** If debugging was hard, add tooling that makes it easier next time—for both humans and LLMs. A well-placed log statement, a clearer error message, a helper function that surfaces state. Small improvements that compound.

## Input

Run with `-c` to include clipboard content:
```bash
lf debug -c
```

Parse the error/stacktrace. Identify file and line. Check if the file was changed on this branch:
```bash
git diff main...HEAD -- <file>
```

## Debugging strategy

**Follow the stack trace.** The deepest frame in your code (not library code) is usually where the problem originates. Start there.

**Check recent changes.** If the error is new, the bug is likely in the delta.

**Reproduce first.** Before fixing, understand how to trigger the error. A fix you can't verify isn't a fix.

## Output

Fix the bug directly. If the cause isn't obvious from the fix, add a brief inline comment.

If you can't determine the cause, describe what you learned and what additional context is needed.
