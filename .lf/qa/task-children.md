Read when the change touches parent and child Tasks: filing from a conversation, notices to a conductor, a merged parent, closing one.

# Parent and child Tasks on a private Machine

No lf-world and no login needed for most of it. Build the fake GitHub
repository of `stacked-tasks.md` (bare origin, `gh-standin.py` as `gh`), and
add `.lf/qa/claude-standin.py` to the same `bin` as `claude`,
`default_agent: claude` and `graph: qa` in `.lf/config.yaml`, and
`.lf/graphs/qa.yaml` (`- cmd: help task`, `- interactive: demo`), committed and
pushed. Isolate `HOME` (with a `.gitconfig` identity), `LF_HOME`,
`CLAUDE_CONFIG_DIR` and `CODEX_HOME`, set `LF_BIN`, and start `lf server run`.

`lf task checkout <task>` starts the Task's conversation on the stand-in under
`lf holder run`. Drive it with
`.lf/qa/pty-drive.py '<steps>' lf session resume <session>`: a line starting
with `!` runs as the agent's Bash tool call, with the conversation's
`LF_AGENT_CALLER`, so `!lf task create "…"` files a child the way an agent
does. The stand-in runs lf's hooks from `--settings`, so each prompt prints the
`<lf:steers>` block it would inject: that is where a notice shows as
delivered. `lf session steers <session> --json` shows it pending before.

- The stand-in has no inbox, so the server never wakes it: a notice is written
  and delivered at the conversation's next prompt. For the wake itself use real
  Claude (below).
- `lf task run <child>` returns once the Graph registers and the `qa` Graph
  stops at `demo` within seconds. `lf task wait` waits for a terminal state,
  not for the stop: do not use it here.
- A failing step: `- cmd: task show nosuchtask` (exits 1; `task status
  nosuchtask` exits 0). `lf task run` relaunches a failed Graph twice, so one
  failure is three runs.
- A merge: publish with `lf pr open --title … --body …` in the Task's checkout,
  act as GitHub (merge the head into `main` on origin, delete the branch, set
  `state: MERGED`, `merge_commit` and `merged_head` in `$GH_QA_STATE`), then
  `lf pr reconcile`. Until that reconcile lf does not know.
- `lf task retire <task>` does nothing, silently, until its fifteen minutes
  pass, and in a private home they count from the first `lf task retire <task>`
  after completion (nothing else runs the pass), restarting when a viewer
  leaves. Complete early, run `lf task retire <task>` at once, and read
  `task_retirements.due_at` for when the second call will take the checkout.
- For a hook's cost, set `CLAUDE_QA_LOG`: the stand-in logs each launch's
  `LF_*` environment, enough to run `lf __context-block --moment tool` by hand
  with Claude's callback JSON on stdin and time it.
- Never `pgrep -fl` for `lf …`: other agents' argv holds whole prompts that
  match. List `ps -axo pid,command` and match the binary path in column two.
- A returning Machine: build the home with the installed `lf`
  (`whence -p lf`), stop its server, `rm` the copy and `cp` the branch binary,
  start the branch server. The old holder keeps the conversation; its hooks run
  `LF_BIN`, now the branch.

# The wake, with real Claude

Keep the real `HOME` (the Keychain login), isolate `LF_HOME`, run with
`env -i` so no `CLAUDE_CODE_*` or `LF_*` leaks in, and put the branch `lf`, the
`gh` stand-in and the real `claude` on `PATH`. `default_agent: claude:haiku`
keeps turns cheap. A repository Claude has not seen stops the first launch at
"Adding a new directory … Yes, add it": the conversation lists as `detached`
with no turn until a viewer answers it (Down, Enter through `pty-drive.py`,
which needs its 40x120 window or Claude draws nothing). Then
`lf steer <parent> 'run exactly: lf task create "…"'` wakes it to file the
child, and `lf task run <child>` wakes it again about two seconds after the
child stops. Read both in
`~/.claude/projects/<checkout with / and . as ->/<thread>.jsonl`: a `user`
entry holding `<cross-session-message from="loopflow">`, then its reply.
Afterwards stop that home's `lf server run`, its `lf holder run` and the
`claude` children under it, by PID after checking `LF_HOME` with `ps eww`.
