Read when the change touches how Antigravity (`agy`) launches, its hooks, or what a Task step or conversation on it records.

# Real `agy` on a private Machine

Real `agy` (signed in under the account's real `HOME`), branch `lf`, isolated
`LF_HOME`. The default model is free-tier Gemini; a step costs nothing here.
Set up as `.lf/qa/opencode-pi-workers.md` does (repo with a bare origin,
`.lf/graphs/qa.yaml` of `- step: implement` then `- interactive: ship-review`,
`graph: qa`, `lf server run` with stdin closed), then:

```bash
lf task create '<request>' --json < /dev/null
lf task run <id> -a agy < /dev/null   # returns once the Graph registers
lf steer <id> '<direction>'              # ~15 s in; it lands after the next tool
lf task wait <id> --until terminal --timeout 150
lf history usage --task <id>
```

`lf task run` also starts the Task's own conversation: an `agy` TUI in a held
terminal, a child of `lf holder run`. Both are on Antigravity when `-a` says so.

What to read afterwards:

- `agy`'s own record: `~/.gemini/antigravity-cli/brain/<conversation>/.system_generated/logs/transcript_full.jsonl`.
  `SYSTEM_MESSAGE` rows hold what lf's hook injected (context block,
  `<lf:steers>`); the conversation id is `agent_sessions.provider_thread`.
- Which thread an input can resume: `session_events.provider_thread` where
  `kind='captured'`. `lf resume` reads that, not `agent_sessions`.
- What the hooks saw: the launch folder is
  `<checkout>/.lf/prompts/<stamp>-<rand>-agy/` (`hook.sh`,
  `.agents/hooks.json`). `agy` runs the script fresh each time, so adding a
  `printf … >> trace.log` line to a live launch's `hook.sh` traces its calls
  and lf's replies.
- An interactive conversation's turn rows read `running` with no status until
  the TUI exits; `lf history usage --task` shows `-` for it. Claude's do too.

# Driving the TUI

A view is `lf resume --task <id>` (or `lf session resume <session>`) under a
Python `pty.fork()` that sets a window size, answers `ESC[6n`, drains output
and types keys at set times; render the saved bytes with
`uv run --with pyte`. Enter is `\r`; quit is ctrl+c twice (`\x03`, a second
apart); Esc first dismisses the feedback prompt `agy` shows after a turn.
Killing only the PTY child leaves a chat's `agy` running: kill it by pid.

- The process list shows the launch: `--conversation <id>` on a resume, no
  `-i` then; `--add-dir <main>/.git` and `--settings {"trustedWorkspaces":…}`
  in a worktree lf created.
- `agy` sandboxes a terminal's commands (not a headless run's): they reach
  workspace folders, `/tmp` and its own `brain/` only. A checkout under `/tmp`
  hides every sandbox failure, so put the repository under the home directory
  (`~/lfq-…/repo`, then `lf wt create <name>`) to see what a person sees.
  Tell the model `with BypassSandbox false … do not retry` to keep it inside.
- `lf` called from inside the sandbox fails on the Machine's files; the model
  retries outside it, which `agy` either approves itself or asks about
  ("Run this command?"). Answer `2` to allow `lf` for the conversation.
- A Task update arriving with a typed prompt can pull the model off the
  prompt; it still answers.

# Switching conversations in the TUI

In a worktree from `lf wt create`, `lf default '<prompt>' -a agy`
needs no server. Type the command, pause a second, then Enter:

- `/new` (the menu lists it as "/clear (new)"), then a prompt.
- `/fork` prints "To go back, use: /resume <id>"; typing that line switches
  back inside the same `agy`. Bare `/resume` opens a picker of every
  conversation on the Machine; Esc closes it.
- A subagent: "Use your invoke_subagent tool to start exactly one subagent
  whose task is: run the shell command `echo sub` and report its output."
  Its conversation is the newest directory under `brain/` afterwards.

A prompt that asks "which Session id does your loopflow context block name"
shows on screen that the block arrived; a `SYSTEM_MESSAGE` row holding "This
Session" in that conversation's transcript shows it was injected there and
not copied by a fork. After ctrl+c twice, `agy` prints the conversation it
quit in ("agy --conversation=<id>"); `agent_sessions.provider_thread` and the
`captured` event should name the same one, and `lf resume <session>` starts
`agy --conversation <id>`. For a Task's conversation, attach with
`lf resume --task <id>` and send `lf steer` while a `sleep 25` tool runs.

Wait for an answer by a line of its own (`(?m)^\s+back`), not a word: the
spinner line prints the model's thinking, which repeats the words of a steer
before the answer exists.

# Steering a quit conversation

A quit chat (`lf default '…' -a agy` in `lf wt create`'s worktree, ctrl+c
twice) or a Task's quit primary is started by the server when `lf steer`
names it: a child of the private `lf holder run` running
`agy --conversation <id> … -i <lf:steers>…`, so `ps -o command= -p <pid>`
shows which steers it took. The steer is the next `USER_INPUT` row in that
conversation's `transcript_full.jsonl`. To save steers while nothing starts
them, stop the server by PID first; the start comes on `lf server run`.

- Started from an agent's tool shell, `lf server run &` keeps running, but
  `ps` shows it as plain `lf server run`; find it with
  `lsof -t <socket from lf server status>` and check `LF_HOME` with
  `ps eww -p <pid>`.
- A PTY viewer of a held terminal (`lf resume S`, which execs
  `lf session attach --terminal …`) ignores SIGHUP; stop it with SIGTERM.
  Quitting agy in it (Esc, ctrl+c twice) ends only agy; the holder stays.
- A quit chat outside any Task is missing from `lf session list --all`; take
  its ID from agy's quit line ("Continue with: lf resume <id>").

# Reopening with waiting steers

Run a Task whose Graph stops at once (`- cmd: help task`, `- interactive:
demo`) with `-a agy`; its conversation's first turn reports status. Quit it
from a view (Esc, ctrl+c twice), then save direction that names no
conversation: `lf --repo . steer '<ask for one word>'`. Nothing starts and
`lf session steers <session>` shows it pending. `lf resume --task <id>` then
starts `agy --conversation <id> … -i Read the waiting steers and Task updates
for this Session.`; the screen shows that line as a turn and the one-word
answer before anything is typed, and the steer reads `delivered`. A second
reopen with nothing waiting has no `-i` and adds no `USER_INPUT` row. Wait
for the answer line before Esc: an Esc while the model still reads the steer
file interrupts the turn, and the steer still reads `delivered`. `lf comment <task>` names the Task,
so the server starts the closed conversation itself with the same line.
While agy is open and idle, a repository steer stays pending until the next
typed turn (LOO-1099).

# Failures on demand

- Unserved model: `lf set --global agents 'agy:no-such-model-xyz,agy'`,
  then `lf task run <id>` with no `-a`. `lf task status` reads "went on under
  agy after: agy_error: invalid model selection …" while the
  step runs; the row is in `agent_limit_recoveries`. Clear
  `$LF_HOME/config.yaml` afterwards: every later launch, a resumed
  conversation included, takes the first agent.
- Interrupt: `lf task interrupt <id>` mid-step; `turns.status` is
  `interrupted` and no `agy --input-format` process is left.
- Signed out: give `lf` an empty `HOME` (`HOME=/tmp/…/fakehome lf -b default
  '…' -a agy`; `LF_HOME` stays set). `agy` answers "authentication
  failed or timed out" and lf prints the sign-in advice. For the move, `lf set
  --global agents 'agy,opencode:opencode/gemini-4-carbon'` and add
  `XDG_DATA_HOME=<real home>/.local/share` so OpenCode keeps its login.
- OpenCode's unserved model: `-a opencode:opencode/no-such-model-xyz`
  ("Model unavailable: …"); first in `agents`, a Task step goes on under the
  next entry.
- `--yolo` in the TUI: `lf default 'Run exactly this one shell command …: lf
  session list' -a agy --yolo` in a worktree under the home directory
  runs `lf` once; without the flag the screen shows the command twice (the
  sandboxed failure, then the retry outside).
- Not reachable here: a spent quota, a Machine without `agy` (lf's own PATH
  adds `~/.local/bin` and `/opt/homebrew/bin`), and an OpenCode balance
  refusal while the account has credits. lf accepts only `opencode/*` models
  outside the Claude and Codex families, and a project `opencode.json` with
  `provider.opencode.options.baseURL` is read (`opencode debug config`) but
  OpenCode 2.0.32 still calls its own endpoint, so a local server cannot
  stand in for the refusal.

Stop the holder's children (`pgrep -P <holder>`), the holder and the server
by pid when done; check a server's `LF_HOME` with `ps eww -p <pid>` first.
The holder's environment may not show `LF_HOME`; find it by its binary path
(`pgrep -f '<Q>/bin/lf holder run'`).
