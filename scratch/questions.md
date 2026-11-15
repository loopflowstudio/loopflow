# Open choices

- Crash follow-up. The brief: "otherwise record a concrete linked follow-up and
  keep this recovery fix shippable." Inspection found no crash record and no
  relaunch in the launch journal for PID 11516, only main-thread hangs of 12.9 s
  and 3.5 s. Choice left open: file a linked Task to find why Desktop exited (or
  whether it hung rather than crashed) during LOO-906, or state the unknown cause
  in the PR. Until decided, the PR body states it.
