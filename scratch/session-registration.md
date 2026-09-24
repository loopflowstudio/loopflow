# Session registration counterexample

Observed 2026-09-23 in the user's supplied screenshot: the Loopflow window displays two embedded Codex design conversations, in `loopflow.lf-new` and `loopflow.main-view`. Both sidebar rows show ELSEWHERE and the indistinguishable title `<lf:skill:design>`.

Source explanation: `SessionsStore.reconcile` checks only `.session(record.id)` in the surface pool. `New shell` creates a `.shell(pane.id)` surface. A Run subsequently launched inside that shell is discovered as a Session, but there is no association to the already-visible shell surface. This explains the screenshot without assuming the provider is detached or dead.

Required result: each row focuses its existing terminal, reports local viewing accurately, and identifies its worktree. Never fix this by automatically invoking Move here: that can stop the very client the user is typing into. A shared cwd alone cannot establish terminal ownership.

Additional user requirement: each worktree groups its LLM conversation(s) and companion shells. Selecting another worktree restores the entire group. A shell's later `cd` does not change its group. Registration and grouping have distinct evidence: checkout identity establishes the group; live client/terminal identity establishes where the conversation is actually displayed.

No sessions were moved, stopped, or restarted during this investigation. The original user screenshot remains in the conversation; the research fixture screenshot is not a substitute for this observation.
