---
requires: an error, unexpected behavior, or a debugging question
produces: the cause, a targeted fix, and the original workflow working again
action_style: procedural
---
Find why the code misbehaves, fix the cause, and see the original workflow work again.

```bash
lf debug -c
lf debug : "Why does this request fail only after reconnecting?"
```

Start from what you were given: a description, stack trace, logs, reproduction
or clipboard. Ask for more only when you can't make progress without it. A
workaround unblocks work; it doesn't fix the cause. When one is in place, find
the cause behind it and replace it with the real fix.

1. **Reproduce it before changing anything.** Get a command that fails, and
   keep its output. A fix made before the failure was seen is a guess. Read the
   stack frames, state transitions and recent changes against what was
   expected. When reproducing would risk data or service, work from the
   original observation and say that you did.
2. **Name the leading explanations and what each predicts.** Run the smallest
   experiment that tells them apart. Keep what you saw apart from what you
   suspect, and drop an explanation the results contradict.
3. **Fix the cause,** within the scope asked. Add a diagnostic or regression
   test when it makes this failure easier to read or stops it coming back.
   Asked only for a diagnosis, explain the cause and the fix without editing.
4. **Rerun the same command** and nearby regressions. Follow any newly
   exposed failure until the workflow succeeds. One green check is not enough
   while other counterexamples exist.

Report the cause, the change, and the same command failing before and passing
after, with the output of both. If the cause is
still unknown, say what you ruled out and the next check worth running. What a
one-off failure showed can't be found again; add it to the existing issue or
note.
