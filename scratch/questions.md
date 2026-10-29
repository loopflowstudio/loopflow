# LOO-683 questions and assumptions

- **Read storm follow-up (open, for Jack Heart).** The Task-file comparison
  feedback loop (about 56k SIGKILLed `lf task diff` reads overnight on
  2026-10-28/29) is excluded from this Task. Should it become its own Task? It
  may also feed the LOO-678 syspolicyd exec storm. Assumption: separate Task; it
  is not filed from this run.
- **Store frontier at launch (assumption).** Desktop records the build
  revision, not the applied store migration. `lf self doctor --json` costs about
  5.7 s of CPU, and Desktop must not read SQLite itself. Doctor reports the
  current store frontier at read time.
- **Second launch at 07:29:44 (open).** The launch journal shows a 0.15.0 launch
  90 s after the relaunch while PID 9446 stayed alive. Its source is unknown: a
  second instance, `open -n`, or a test. Unstamped `"app":"1.0"` bundles also
  write to the shared journal (Oct 28 16:48 and Oct 29 08:05 PDT). The new `pid`
  and `bundle` fields will identify the next one.
- **Exit caller (open).** `exit(1)` at 07:27:25, and again at 08:00:11, has no
  identified caller. The
  candidates are libghostty and the WebKit/AppKit stack, right after a scratch
  WKWebView page closed. The atexit stack ships to answer it next time.
- **Quit before a Force Quit (assumption).** The concept review listed abnormal
  ends plus the latest end, which hides a ⌘Q followed by a Force Quit; the live
  acceptance needs both. Doctor also lists the last quit within 24 h.
