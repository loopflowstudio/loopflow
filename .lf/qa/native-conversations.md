Read when the change wakes idle conversations or opens them natively (Rex).

# Waking idle conversations

Real turns need real logins: keep the account's `HOME` (Claude's Keychain login
and `~/.codex`) and isolate only `LF_HOME`, with the branch `lf` first on `PATH`.
Interactive `-i` launches need no lf account. `lf checkout <task>` itself starts
the Task's primary Claude conversation, which runs one `task-session` turn and
goes idle: that alone is a managed idle conversation. Start Codex with
`lf --task <task> -i -a codex : "Reply only with QA_OK"` in a PTY; its first turn
needs no typing. A conversation is a wake candidate once
`session_activity.yielded` is 1 and `processes.inbox` (Claude) or
`processes.endpoint` (Codex) is set. Then `lf comment <task> 'Reply only with X'`
and read the reply: Claude's in
`~/.claude/projects/<cwd with / as ->/<thread>.jsonl` (a `queue-operation`
then a `user` cross-session message), Codex's through its app-server
(`thread/turns/list` with `itemsView: full`). A Codex client may sit on an
update dialog; its app-server still runs the turn. Kill the private server's
PID (find it by `LF_HOME` in `ps -E`) to check news recorded while it was down.
`ps` matches `__context-block` in Codex app-server argv: those are hook
declarations, not processes.

Start that private server with `CLAUDECODE`, `CLAUDE_CODE_*` and `AI_AGENT`
unset as well as `LF_*`: from an agent's shell its held Claudes inherit
`CLAUDE_CODE_CHILD_SESSION`, write no `~/.claude/sessions/<pid>.json`, so the
idle wake finds no inbox and skips them silently, and their turns may not reach
the transcript. A fresh repository's main checkout is not trusted, so every
Claude lf starts with `--add-dir <main>` stops on Claude's "Adding a new
directory" dialog and writes no `<pid>.json` until it is answered; trust it once
by running `claude` there and choosing "Yes, I trust this folder".

# Starting a stopped conversation

Prepare a Session (`lf -a claude session resume --prepare --json`) and give it
turns with `lf -b steer`; then, with the server running, `lf steer <id> TEXT`
starts it in a held terminal: `pgrep -f 'resume <thread>'`, its reply in the
transcript, the steer `delivered` in `lf session steers <id>`. Each steer starts
its Session once: a second start needs a new steer. For a Task, `lf checkout`
starts the primary conversation; wait for its first turn, kill its Claude, then
`lf steer <task> TEXT`. Killing a Claude before its first turn leaves an
assigned thread Claude never wrote, and every later resume exits with "No
conversation found with session ID". Attach with the PTY viewer
(`lf resume <id>`, answering `ESC[6n`/`ESC[5n`) to see a start that is waiting.
With `LF_BIN` on the branch, `lf checkout` starts that home's server itself
when none runs, so a "server down" scenario ends at the first checkout. Before
restarting, find every server for the home (`ps -E -ww -p <pid>` showing
`LF_HOME=/private/tmp/...`; the macOS path is `/private`): `lf server status`
stays ready while any one of them runs.

# Native conversation opening

Use the private homes above for native servers too. Start Rex with
`rex server run --listen unix://<root>/rex.sock --state-dir <root>/rex-state
--config <root>/empty.lua`; create an empty Lua file first. Every Rex client
command takes `--autostart=false -S unix://<root>/rex.sock`. Never launch a second
Rex app: its preferences share the installed app's cached route. A headless Rex
server can attach and capture real terminals; focus correctly fails without a
capable display client.

Open with `lf open --tab --host rex --server unix://<root>/rex.sock --json`.
Use `rex api call session.list_blocks '{"session_id":"…"}'`, then
`rex block capture -s <session> -b <block>` to read the terminal. To check the
direct default inside Rex, create a `session.new_window` whose terminal command
is `["<branch-lf>","open","--task","<task>"]`; it must stay in that block.

For Herdr, isolate `XDG_CONFIG_HOME` and `XDG_STATE_HOME`, start
`herdr --session qa server`, and set `HERDR_SOCKET_PATH` to
`<config>/herdr/sessions/qa/herdr.sock` in clients. `herdr pane list` and
`herdr pane read <pane> --source visible --format text` show the real destination.
Two simultaneous opens of one prepared Session should create one attachment.
`herdr workspace create --cwd <repo>` makes the calling pane; to act as an agent
tool call from it, set `HERDR_ENV=1`, `HERDR_PANE_ID`, `HERDR_WORKSPACE_ID` and
`LF_AGENT_CALLER` to `{"session_id":…}` of a Session prepared with
`lf session resume --prepare --json` (process IDs may be any UUIDs). Opening
then takes the sibling-tab path. In zsh, kill a PID list with `kill ${=pids}`:
an unquoted variable is not word-split.

To exercise failed-start recovery with real Codex, exit its private sign-in
screen with Ctrl+C, temporarily put invalid TOML in the private Codex config,
and reopen that Session. Capture the retained error tab, restore the config,
then repeat the exact opening command. Check the Session and provider-thread
identity, and capture again after startup settles. Attachment at the sign-in
screen does not establish authenticated conversation behavior. Stop native
servers and their terminal descendants, including detached owners recorded in
the private store.
