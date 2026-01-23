## 2026-01-23

- No error provided in clipboard - clipboard contained recursive copy of lf debug context. Need actual error/stacktrace to debug.
- Second attempt: clipboard still contains lf debug context, not an error. Copy the actual error to clipboard before running `lf debug -c`.
