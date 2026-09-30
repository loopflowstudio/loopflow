---
requires: an error, unexpected behavior, or a debugging question
produces: a supported diagnosis, targeted fix and verification
action_style: procedural
---
Find why the code misbehaves, fix the cause, and verify the original workflow.

Use the evidence already supplied: a description, stacktrace, logs, reproduction,
or clipboard content. Ask for missing information only when it prevents progress;
clipboard input and a Task are optional.

```bash
lf debug -c
lf debug : "Why does this request fail only after reconnecting?"
```

1. Establish the expected and observed behavior. Reproduce the failure safely;
   inspect relevant stack frames, state transitions and recent changes. Preserve
   the original observation when reproduction would risk data or service.
2. State the leading explanations and what each predicts. Trace execution and
   run the smallest useful experiment that distinguishes them. Keep observations
   separate from hypotheses. Revise the explanation when evidence contradicts it.
3. Repair the supported cause within the requested scope. Keep the change
   targeted; add a useful diagnostic or regression check when it makes this
   failure easier to understand or prevents its recurrence. For an explicit
   diagnosis-only request, explain the cause and proposed repair without editing.
4. Replay the original reproduction and relevant regressions. Follow newly
   exposed failures until the requested workflow succeeds. Verify against all
   relevant counterexamples, not only the latest green check.

Report the cause, change and observed proof. If the cause remains unknown, state
what was ruled out and the next useful check. Keep consequential evidence with
the existing issue or working note; do not require a new report for every bug.
A workaround may unblock the caller, but does not establish that the cause is fixed.
