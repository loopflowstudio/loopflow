# break-test wave memory

## Learnings

- 2026-04-22: `lf debug` invoked headlessly on this branch with no
  clipboard content and no explicit error target. Step's documented
  behavior ("ask what error to debug") is incompatible with headless
  mode. Logged to `scratch/questions.md` and exited without changes.
  If this recurs, consider patching `.lf/steps/debug.md` to detect
  headless + empty input and exit cleanly with a one-line message.
