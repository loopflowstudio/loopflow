Read when the change touches how OpenCode or Pi run a Task step, or what their plugin or extension delivers.

# Real OpenCode and Pi workers on a private Machine

Real CLIs, real models, branch `lf`, isolated `LF_HOME`, the account's real
`HOME` (OpenCode keeps its login in `~/.local/share/opencode/auth.json`, Pi in
`~/.pi/agent/auth.json`). `opencode/gemini-4-carbon` works for both:
`-a opencode:opencode/gemini-4-carbon`, `-a pi:opencode/gemini-4-carbon`
(`pi --list-models gemini`). A step costs about $0.20.

```bash
Q=/tmp/lfq; mkdir -p $Q/bin $Q/home; cp target/debug/lf $Q/bin/lf
# repo with a local identity, a bare origin, and this Graph:
#   .lf/graphs/qa.yaml:  - step: implement
#                        - interactive: ship-review
#   .lf/config.yaml:     graph: qa
for v in $(env | grep '^LF_' | cut -d= -f1); do unset $v; done
export LF_HOME=$Q/home LF_BIN=$Q/bin/lf PATH=$Q/bin:$PATH
lf server run < /dev/null > $Q/server.log 2>&1 &     # a worker needs it
lf task create 'Add a divide(a, b) function to calc.py' --json < /dev/null
lf task run <id> -a opencode:opencode/gemini-4-carbon < /dev/null
sleep 12; lf steer <id> 'Also give divide a docstring "Return a over b."'
```

A Graph item is `step:`, not `skill:`; `interactive:` names a skill that
exists (`ship-review`). Poll `lf task status <id>` for `at ship-review`.

What to read afterwards:

- OpenCode's own record: `sqlite3 "file:$HOME/.local/share/opencode/opencode.db?mode=ro"`,
  table `session_v2` (`id`, `directory`, tokens) and `session_message`
  (`session_id`, `seq`, `type`, `data`). `directory` must be the Task checkout.
  OpenCode takes it from `$PWD`, not the process cwd.
- A steer the plugin claimed: `steer_delivered` in `$LF_HOME/loopflow.db`;
  `.lf/prompts/*-steer.md` in the checkout. Plugin context goes into the
  request's system text, which OpenCode does not store, so a steer is never in
  `session_message`; read the code the model wrote.
- Usage: `lf history usage --task <id>`.
- Interrupt: `lf task interrupt <id>` a few seconds in; the worker's turn reads
  `interrupted` in `lf session history <worker session>`.
- Not installed: run `lf task run` with a `PATH` lacking the CLI (Pi lives
  under `~/.nvm/.../bin`, OpenCode under `/opt/homebrew/bin`). For a move,
  `lf set --global agents pi:…,opencode:…`; the move row is in
  `agent_limit_recoveries`, and status shows it only while the step runs.

- A real question: Gemini declines to ask when told only "ask me…; do not
  guess" (the headless instructions win). This brief makes it call the tool:
  `'This Task tests the question tool. Your first and only action must be a
  call to the \`question\` tool asking: "…?" Call it even though the run is
  headless.'` Then `lf steer <id> '<answer>'` and `lf task run <id>`.
- A real Pi question: Pi has no question tool, and headless Pi skips
  untrusted project `.pi/extensions/`. Copy `~/.pi/agent/{auth,settings}.json`
  into `$Q/piagent/` (mode 700, delete after), add
  `$Q/piagent/extensions/ask.ts` registering a tool whose `execute` calls
  `ctx.ui.select(question, options)`, and export `PI_CODING_AGENT_DIR=$Q/piagent`
  for both `lf server run` and `lf task run`. Brief: "Your first and only
  action must be a call to the `ask` tool …".
- A Pi model failure on demand: in the same agent dir, a `models.json`
  provider (`"api": "openai-completions"`, dummy `apiKey`) whose `baseUrl`
  is a local server answering 429; run `-a pi:<provider>/<model>`. The step
  fails three times (lf's Graph retry) and each `turns.error` reads
  `pi_model_error`.
- Kill a Pi worker mid-turn: its process title is `pi`, a child of the
  `lf --batch … skill implement` process; `kill -9` it. The turn reads
  `pi_disconnected`, then lf retries the Graph.
- Claude and Codex workers interrupt the same way (`-a claude`, `-a codex`);
  read `turns.status` in `$LF_HOME/history.db`. Codex can hit the account's
  real usage limit, after which the step waits with "waiting for codex
  capacity"; `lf task interrupt` ends that wait.
- Holder cleanup: `lf holder run` under `$Q` keeps each Task conversation
  (claude, codex, opencode, pi) as its child; `pgrep -P <holder>`, then kill
  those and the holder.

Run from an agent shell, lf inherits that shell's caller markers
(`OPENCODE_SESSION_ID`, `OPENCODE_CONFIG_CONTENT`, `CLAUDECODE`); keep them, as
they are the case a person meets. Stop the server, and any `opencode` or `pi`
under `$Q`, when done: `lf holder run` and the Task conversations outlive it.
