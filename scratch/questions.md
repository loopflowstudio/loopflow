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
- A headless Antigravity turn always passes
  `--dangerously-skip-permissions`, whatever `yolo` says, as OpenCode's
  always passes `--auto`. The other choice is to follow `yolo` and let a
  refused permission fail or stall the step.
- An Antigravity failure's reason is the first line of what `agy` said. The
  rest (the list of models it does serve) is in `antigravity.stderr.log`
  beside the run, not in `lf task status`.
- A `result` with `status: SUCCESS` and an empty `response` completes the
  turn with no answer. OpenCode's driver fails a turn that has no answer.
- Every headless Antigravity turn gets the context block again: each turn is
  a new `agy` process and its first hook callback is a start. OpenCode does
  the same. The other choice, a start only for a new conversation, leaves a
  resumed process unable to report turns or claim steers.
- `lf __context-block` with no `--harness` reads the callback as Claude's
  shape, which Codex, Pi and OpenCode's hooks also send. Naming the harness on
  every hook would change the declarations Codex asks a person to trust.
- `agy` is given `hook.sh` in the launch folder, beside `.agents`, not inside
  it: what `agy` does with an unknown file in `.agents` was not checked.
