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
`LF_AGENT_CALLER`, so `!lf task create --parent <parent> "…"` files a child the way an agent
does. The stand-in runs lf's hooks from `--settings`, so each prompt prints the
`<lf:steers>` block it would inject: that is where a notice shows as
delivered.

- The stand-in has no inbox, so the server neither wakes it nor writes its
  notices: a notice is written and delivered at the conversation's next
  prompt, and `lf session steers <session> --json` reads `pending: 0` until
  then. Send `!true` to take a reading. For the wake itself use real Claude
  (below).
- `lf task run <child>` returns once the Graph registers and the `qa` Graph
  stops at `demo` within seconds. `lf task wait` waits for a terminal state,
  not for the stop: do not use it here.
- A failing step: `- cmd: task show nosuchtask` (exits 1; `task status
  nosuchtask` exits 0). `lf task run` relaunches a failed Graph twice, so one
  failure is three runs. Keep it in a second Graph file (`qa-fail.yaml`) and
  pick per run: `lf task run <child> qa-fail`, then `lf task run <child> qa`
  moves the child to `demo`, which is what makes the same failure news again.
- A Graph that reaches `end`: `qa-end.yaml` holding only `- cmd: help task`.
  Run on a childless Task it completes it; on a Task with an open child it
  leaves it open.
- A parent whose own Graph ends: `lf task run <parent> qa-end` with a child
  open. Reaching `end` leaves it open: `lf task status <parent>` reads
  `Finished · 1 child open`, and its `--json` `.task.reason` matches the text
  `action:` line.
- Wait on output, not time, before depending on what a `!` line did: a
  conversation that is still starting runs the line seconds after the driver
  has left. `["wait","parent: [0-9a-f]{6}",20]` after each `!lf task create`,
  and read `lf task status <parent>` for the children before running its Graph.
- Served writes need no client: after `hello`, send
  `{"kind":"task.create","directory":…,"request_id":<uuid>,"title":null,"request":"…","wave":null,"parent":"<id>","graph":null,"session":null,"run":false}`
  and `{"kind":"task.complete","directory":…,"issue":"<id>"}` in the envelope
  `served-launches.md` describes. An unknown `wave` creates that Wave, as
  `--wave` does.
- A second Machine: a second `HOME`/`LF_HOME` on a fresh clone of the bare
  origin (`tracker.md`); its first `lf task list` shows the link. It also
  lists Tasks abandoned on the first Machine as open: an arriving `canceled`
  sets no `abandoned_at`, on main too.
- Do not attach a viewer to a completed Task's conversation while waiting out
  its retirement: the fifteen minutes restart when the viewer leaves.
- The parent's checkout holds whatever `main` was when it was placed; a Graph
  file pushed later is in new checkouts only.
- A merge: publish with `lf pr open --title … --body …` in the Task's checkout,
  act as GitHub (merge the head into `main` on origin, delete the branch, set
  `state: MERGED`, `merge_commit` and `merged_head` in `$GH_QA_STATE`), then
  `lf pr reconcile`. Until that reconcile lf does not know.
- `lf task retire <task>` does nothing, silently, until its fifteen minutes
  pass, and in a private home they count from the first `lf task retire <task>`
  after completion (nothing else runs the pass), restarting when a viewer
  leaves. Complete early, run `lf task retire <task>` at once, and read
  `task_retirements.due_at` for when the second call will take the checkout.
- A completed Task with no PR is never retired: `lf task retire <task>` exits
  1 with "Retained PR-less checkout", and its conversation reopens in that
  checkout. Use a parent with a merged PR to see the main-checkout resume.
- For a hook's cost, set `CLAUDE_QA_LOG`: the stand-in records launch and hook results.
- Never `pgrep -fl` for `lf …`: other agents' argv holds whole prompts that
  match. List `ps -axo pid,command` and match the binary path in column two:
  that finds the holder (`$LF_BIN holder run`), whose children are the
  providers. The server shows as plain `lf server run`; record its PID at
  start and check `LF_HOME` with `ps eww -p <pid>` before stopping it.
- Never pipe `lf task status` or `lf task list` into `head`: lf panics on the
  closed pipe. Use `sed -n 1,5p`.
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
`lf steer <parent> 'run exactly: lf task create --parent <parent> "…"'` wakes it to file the
child, and `lf task run <child>` wakes it again about two seconds after the
child stops. Read both in
`~/.claude/projects/<checkout with / and . as ->/<thread>.jsonl`: a `user`
entry holding `<cross-session-message from="loopflow">`, then its reply.
Afterwards stop that home's `lf server run`, its `lf holder run` and the
`claude` children under it, by PID after checking `LF_HOME` with `ps eww`.

# Completing a parent: `--children`, live children, reopen

Same fake-GitHub world. `lf task create '<words>' --parent <id> --json` builds
the tree; zsh does not split `${2:+--parent $2}`, so pass `--parent` literally.

- A live child without a provider: commit `.lf/graphs/live.yaml` holding
  `- cmd: task wait <some open Task>` before the child's checkout is placed,
  then `lf task run <child> live`. Its Graph stays `running` until that Task
  closes, which is what `--children abandon` refuses on.
- `lf task interrupt <child>` answers "stopped Graph live at task wait …" but a
  running `cmd:` step never reads the interrupt; the processes keep going and
  the refusal stays. Stop them by PID (`lf task run` carrier first, then its
  `graph run` and the step) from `ps -axo pid,ppid,command`, matching the
  private `bin/lf` path. Status then reads `Stopped … exited with status 130`.
- A merged child: commit in its checkout, `lf pr open`, merge its branch into
  `main` on origin, delete the branch, set `state: MERGED`, `merge_commit` and
  `merged_head`, then `lf pr reconcile` in that checkout.
- Abandonment deletes the branch and checkout. The stand-in answers
  `headRepository` (without it lf refuses "open PR has unavailable head
  repository") and `pr close`, but does not close a PR when its head branch is
  deleted the way GitHub does: the state file keeps it `OPEN`, so a later
  `lf pr open` reuses that number instead of opening a new one.
- Reopen, then `lf task run <child>`: after abandonment deleted the checkout and
  branch, the new checkout starts from current main and the Graph runs.
- Refusals write nothing: diff `select … from tasks` and `count(*) from
  task_events` before and after.
- Served `task.complete`: hello as `"kind":"desktop"` (a `machine` client must
  carry a sender), send `"children": null | "rehome" | "abandon"`. A refusal
  answers `code: conflict`; a missing `children` key is `invalid`; an unknown
  policy reads "this server has no operation `task.complete`", the message a
  newer client gets from an older server.
