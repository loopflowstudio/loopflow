# Open questions — break-test debug run (2026-04-22)

## No error to debug — resolved

Second occurrence of `lf debug` invoked headless with no clipboard
content and no explicit target (first: branch `...20260422_1307`,
second: branch `...20260422_1308`). The builtin step says to "ask what
error to debug," which is incompatible with headless mode.

**Resolution applied this run:** added `.lf/steps/debug.md` override
that short-circuits on headless + empty input with a one-line message,
rather than entering an ambiguous state. Interactive behavior is
unchanged — it still asks.

No code change beyond the step override. No debugging performed (there
was still no error to debug on this run).
