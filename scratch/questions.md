# Open questions — break-test

## debug invocation with no error input

The `debug` step was invoked headlessly with:
- No clipboard content (no `-c` output in prompt)
- No user-provided error text
- Clean git status on branch `jack-heart.break-test.20260423_1312`

The step instructs "If clipboard is empty or no -c flag, ask what error to debug" — but headless mode forbids questions.

**Assumption:** This is an intentional break-test probe. The correct behavior is to exit cleanly and surface the missing-input condition here rather than fabricate an error to debug.

**Recommendation for the step itself:** `code/debug.md` should have an explicit headless branch — when neither clipboard nor an inline error is provided, emit a structured "no input" signal and exit 0 rather than rely on a question that cannot be asked. Current wording conflates interactive and headless paths.
