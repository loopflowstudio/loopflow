# Open questions — break-test debug run (2026-04-22)

## Nothing to debug

`/debug` was invoked in headless mode with:
- No `-c` flag (no clipboard content)
- No error message in the prompt
- Clean working tree (no recent changes to inspect)
- No `wave/break-test/` directory (wave context is rotation-only)

The `debug` step instructs the agent to "ask what error to debug" when
clipboard is empty, but headless mode forbids asking questions. Without
an error, stacktrace, or symptom, there is no concrete target to fix.

**Assumption:** This is a deliberate break-test exercising the "debug
with no input in headless" edge case. The correct executive call is to
exit cleanly rather than fabricate a target — debugging the wrong thing
produces noise and false confidence.

## What the user should provide on a real run

- `lf debug -c` after copying an error/stacktrace to clipboard, **or**
- An inline error message in the prompt: `lf debug "TypeError: ..."`, **or**
- A failing test name / reproduction command

## Suggestion for the step itself

Consider having `code/debug.md` detect headless + empty input and exit
with a clear `no-input` status rather than relying on the agent to
notice. Today the contract ("ask the user") silently conflicts with
headless mode.
