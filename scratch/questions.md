# Open choices (made for review in plan.md, "Choices made for review")

- Antigravity subagents' tokens are not counted in `lf history usage`; only
  the parent conversation's are. Counting them means reading each subagent's
  own transcript, named in the parent's `invoke_subagent` result row.
- Conversations started with bare `agy` (outside lf) are not listed for
  adoption: the transcript has no cwd. `conversation_summaries.db` has it.
- `agy` is assumed to retry transient provider failures itself, as OpenCode
  and Pi do. Not seen live.
- `-a agy` is not accepted; the agent is `antigravity` and the install hint
  names the command.
- When one turn reports two different provider failures, the gravest wins
  (credential, usage limit, model, transient) whichever came first. Before, a
  coded failure won over a worded one.
- A subagent's first model call starts one `lf` that answers `{}`; the shell
  script does not filter by conversation.
