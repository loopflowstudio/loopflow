---
requires: an error, unexpected behavior, or a debugging question
produces: the cause, a targeted fix, and the original workflow working again
action_style: procedural
---
Find why the code misbehaves, fix the cause, and verify the original workflow.

This is the one place for finding and fixing a cause, for an ordinary bug or
after an incident has been restored. When a workaround is in place, find the
cause behind it and replace the workaround with the real fix where you can.

Start from what you were given: a description, stacktrace, logs, reproduction or
clipboard content. Ask for more only when you can't make progress without it.

```bash
lf debug -c
lf debug : "Why does this request fail only after reconnecting?"
```

1. Pin down expected and observed behavior. Reproduce the failure safely; read
   the stack frames, state transitions and recent changes. When reproducing
   would risk data or service, work from the original observation.
2. Name the leading explanations and what each predicts. Trace execution and run
   the smallest experiment that tells them apart. Keep what you saw separate
   from what you suspect, and drop an explanation the results contradict.
3. Fix the cause, within the requested scope. Keep the change targeted; add a
   diagnostic or regression test when it makes this failure easier to read or
   stops it coming back. If only a diagnosis was asked for, explain the cause
   and the fix without editing.
4. Rerun the original reproduction and nearby regressions. Follow any newly
   exposed failure until the workflow succeeds. One green check is not enough
   when other counterexamples exist.

Report the cause, the change, and the run that shows it fixed. If the cause is
still unknown, say what you ruled out and the next check worth running. When
something can't be found again from the code or a fresh run, such as what a
one-off failure showed, add it to the existing issue or working note; not every
bug needs a report. A workaround can unblock work, but it does not fix the cause.
