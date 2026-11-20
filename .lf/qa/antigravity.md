Read when the change touches how Antigravity (`agy`) launches, its hooks, or what a Task step or conversation on it records.

# Real `agy` on a private Machine

Real `agy` (signed in under the account's real `HOME`), branch `lf`, isolated
`LF_HOME`. The default model is free-tier Gemini; a step costs nothing here.
Set up as `.lf/qa/opencode-pi-workers.md` does (repo with a bare origin,
`.lf/graphs/qa.yaml` of `- step: implement` then `- interactive: ship-review`,
`graph: qa`, `lf server run` with stdin closed), then:

```bash
lf task create '<request>' --json < /dev/null
lf task run <id> -a antigravity < /dev/null
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
  `<checkout>/.lf/prompts/<stamp>-<rand>-antigravity/` (`hook.sh`,
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

# Failures on demand

- Unserved model: `lf set --global agents 'antigravity:no-such-model-xyz,antigravity'`,
  then `lf task run <id>` with no `-a`. `lf task status` reads "went on under
  antigravity after: antigravity_error: invalid model selection …" while the
  step runs; the row is in `agent_limit_recoveries`. Clear
  `$LF_HOME/config.yaml` afterwards: every later launch, a resumed
  conversation included, takes the first agent.
- Interrupt: `lf task interrupt <id>` mid-step; `turns.status` is
  `interrupted` and no `agy --input-format` process is left.
- Signed out: give `lf` an empty `HOME` (`HOME=/tmp/…/fakehome lf -b default
  '…' -a antigravity`; `LF_HOME` stays set). `agy` answers "authentication
  failed or timed out" and lf prints the sign-in advice. For the move, `lf set
  --global agents 'antigravity,opencode:opencode/gemini-4-carbon'` and add
  `XDG_DATA_HOME=<real home>/.local/share` so OpenCode keeps its login.
- OpenCode's unserved model: `-a opencode:opencode/no-such-model-xyz`
  ("Model unavailable: …"); first in `agents`, a Task step goes on under the
  next entry.
- `--yolo` in the TUI: `lf default 'Run exactly this one shell command …: lf
  session list' -a antigravity --yolo` in a worktree under the home directory
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
