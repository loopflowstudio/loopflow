# Questions

## Missing wave directory

`wave/lfd/` does not exist. The worktree name `loopflow.lfd` implies wave `lfd`, but no matching wave plan was found.

Available waves: `agent-embedding`, `chord-model`, `concerto`, `dogfood`, `pm`, `redesign`, `trust`.

**Decision needed:** Should a `wave/lfd/` plan be created, or should this worktree target an existing wave?

## WaveExecutor follow-on timing

The design scopes out refactoring WaveExecutor to spawn `lf` instead of agents directly. This is the big payoff — it eliminates flow logic duplication. But it's a larger change that should land after the event contract is proven. Worth tracking as a fast follow.
