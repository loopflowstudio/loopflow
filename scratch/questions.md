# Open questions — break-test debug run (2026-04-22)

## No error to debug

The `debug` step was invoked in headless mode with no clipboard content
(`-c` flag appears unused or clipboard empty) and no explicit target.
The step's own instructions say:

> If clipboard is empty or no -c flag, ask what error to debug.

Headless mode forbids asking. There is also no `wave/break-test/`
directory — this run appears to be a deliberate exercise of the
no-input edge case on branch `jack-heart.break-test.20260422_1307`.

**Assumption:** this is a no-op run. Exiting without making code
changes. If a real error was intended, re-invoke with `lf debug -c`
after copying the stacktrace to the clipboard, or pass the error
inline.

**Follow-up suggestion for the step itself:** when clipboard is empty
*and* the run is headless, the debug step should exit cleanly with a
one-line explanation rather than entering an ambiguous state. That's
a change for `.lf/steps/debug.md` (or the builtin) — not for this run.
