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
#                        - interactive: human-review
#   .lf/config.yaml:     graph: qa
for v in $(env | grep '^LF_' | cut -d= -f1); do unset $v; done
export LF_HOME=$Q/home LF_BIN=$Q/bin/lf PATH=$Q/bin:$PATH
lf server run < /dev/null > $Q/server.log 2>&1 &     # a worker needs it
lf task create 'Add a divide(a, b) function to calc.py' --json < /dev/null
lf task run <id> -a opencode:opencode/gemini-4-carbon < /dev/null
sleep 12; lf steer <id> 'Also give divide a docstring "Return a over b."'
```

A Graph item is `step:`, not `skill:`; `interactive:` names a skill that
exists (`human-review`). Poll `lf task status <id>` for `at human-review`.

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

Run from an agent shell, lf inherits that shell's caller markers
(`OPENCODE_SESSION_ID`, `OPENCODE_CONFIG_CONTENT`, `CLAUDECODE`); keep them, as
they are the case a person meets. Stop the server, and any `opencode` or `pi`
under `$Q`, when done: `lf holder run` and the Task conversations outlive it.
