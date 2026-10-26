# LOO-602 open questions and assumptions

- Assumption: the design stays a draft and follow-up Tasks wait for Jack's choice among §7 decisions; the Task outcome says "follow-up Tasks for the changes Jack accepts", so none were filed headless.
- Assumption: the LOO-570 prune's effect on this machine is taken from #1666's clone probe (9.8 GB → 550 MB); it has not run here because v0.14.16 is unpublished.
- Open: which readers use stream `usage` observations versus native `usage` receipts (`context_usage.rs`, `lf history usage`, Desktop). Trace before deleting either (decision 2).
- Open: whether Claude's `cleanupPeriodDays` is set in the per-account homes under `~/.lf/accounts/claude/`; it decides how long native transcripts stay readable (decision 4).
