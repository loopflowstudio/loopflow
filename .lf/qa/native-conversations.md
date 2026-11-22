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

For tmux, start a private server from the cleaned environment
(`tmux -L <name> -f /dev/null new-session -d -s caller -c <repo> "zsh -f"`):
panes inherit the server's environment, so one started from an agent shell
leaks its `LF_*` and `REX_*` into every conversation. `lf open --host tmux`
reads `TMUX` and `TMUX_PANE`, so the caller is a pane: `send-keys` the command
with its output redirected to files and wait for a `.rc` file. Address the
caller by pane ID (`-t %0`): `-t caller` is the session's current window, which
is the conversation once a tab opens. With the real `HOME`, `zsh -f` still puts
`~/.cargo/bin` first, where an old `lf` may sit; name the branch `lf` by path.
Claude needs `USER` to find its Keychain login: under a bare `env -i` it starts
and says "Not logged in". Read the new
window with `capture-pane -p -t <destination>` and its command with
`list-panes -a -F '#{pane_id} #{pane_start_command}'`.

tmux gives a window the `PATH` of the client that asked for it, and nothing
else from that client. Every later `new-window` you run for a caller pane goes
through the cleaned environment too, or the pane gets your shell's `PATH` and
its `lf` is the installed one. To check what a conversation was started with:
`ps -Eww` shows the viewer (`lf session attach`) and the holder, but macOS
hides the environment of a `/bin/sh` stand-in, so have the stand-in write
`env > <file>` and read `PATH` and `TMUX_PANE` there. A provider the holder
already runs is not restarted by an open: type `quit` in its pane first.
`--tab` always adds a view; `--focus` reveals the last one opened and needs an
attached client (`tmux attach` in a PTY you drain), else "no current client".
For `--host tmux` from outside tmux, set `TMUX_TMPDIR` to a private directory
on both the default server and the caller.

To open with a provider missing, give the Machine an empty `HOME` (plus
`CLAUDE_CONFIG_DIR` and `CODEX_HOME` inside it): lf adds `~/.local/bin` and the
other tool directories to every `PATH`, so a bare `PATH` alone hides nothing.
Start `lf server run` yourself from that environment before the first open:
otherwise the open registers the private server under launchd, whose `HOME`
is the account's real one, and the holder finds the real providers. Set
`lf config set agent claude` too: with no preference the only installed
harness is taken, and `/opt/homebrew/bin/opencode` is one.
That works in tmux and Herdr. A Rex terminal runs through `login`, which puts
the account's real `HOME` back, so there the real providers are found; open
with a harness no tool directory holds (`-a pi` when Pi is installed under nvm)
to see a failure. `rex` and `herdr` must be on the cleaned `PATH` themselves
(`ln -s` them into the private `bin`). `--json` on a `--here` open prints the
conversation's terminal bytes before the JSON; use `--tab`.

Rebuilding `lf` under a running private server leaves the server on the old
build's holder: the next open fails with "Holder at … is not answering".
Restart the server after every rebuild. A headless `lf -b -a <harness> session
resume <id> --message` is refused while a view is attached ("already has an
attached LfProcess"); move an attached conversation with `lf resume -a
<harness> <id>` in another pane. `lf install server` with a private `LF_HOME`
registers `com.loopflow.server.<hash>` (the hash is in the socket path);
`launchctl print gui/$UID/<label>` shows its environment, `ps eww` of the
holder and provider shows the `PATH` they were given, and `launchctl bootout`
removes it. A killed provider is restarted by the server's recovery, which
registers that job again: stop the conversation's processes, then boot out.
The installation entry gate is found from the OS account, not `HOME` or
`LF_HOME`, and `lf install` takes only published builds: a branch's code
cannot run behind the gate before it is published.

For Herdr, isolate `XDG_CONFIG_HOME` and `XDG_STATE_HOME`, start
`herdr --session qa server`, and set `HERDR_SOCKET_PATH` to
`<config>/herdr/sessions/qa/herdr.sock` in clients. `herdr pane list` and
`herdr pane read <pane> --source visible --format text` show the real destination.
Two simultaneous opens of one prepared Session should create one attachment.
`herdr workspace create --cwd <repo>` makes the calling pane; to act as an agent
tool call from it, set `HERDR_ENV=1`, `HERDR_PANE_ID`, `HERDR_WORKSPACE_ID` and
`LF_AGENT_CALLER` to `{"session_id":…,"origin_exec_id":"<any UUID>"}` of a
Session prepared with `lf session resume --prepare --json`; without
`origin_exec_id` the value is ignored and the open attaches here. Opening
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
