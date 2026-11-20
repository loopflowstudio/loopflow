# LOO-986 verify, 2026-11-19

Main at `1a6c9d618a` (#1992), `cargo build -p loopflow --bin lf`, private `LF_HOME`
under `/tmp/v986`, own `lf server run`, private tmux 3.7c (`tmux -L`).

Outcome: **fix forward**. One claim fails in tmux.

## Fails: an agent's PATH reaches the conversation it opens into a tmux pane

tmux gives a new window the `PATH` of the client that ran `new-window`
(global environment here was `PATH=/usr/bin:/bin:/usr/sbin:/sbin`):

    pane$ export PATH=/tmp/v986/agentshim:$PATH
    pane$ tmux new-window -d 'echo $PATH > out'      -> /tmp/v986/agentshim:/Applications/Loopflow.app/…
    pane$ lf open --task b2ad67 --host tmux --tab --json   -> status usable, destination %3
    ps -Eww of `claude --name b2ad67 …` (child of `lf holder run`):
      PATH=/tmp/v986/agentshim:/Applications/Loopflow.app/Contents/MacOS:/Users/jack/.opencode/bin:…

The pane command itself has no `PATH=`. The leak is `open/tmux.rs` `open`
running `tmux new-window` with lf's own `PATH`, and the provider taking the
viewer's `PATH`. Repaired under "Fix forward" below.

## Holds

- launchd `PATH`, no `PATH=`: server and pane started with
  `env -i HOME USER TERM LF_HOME PATH=/usr/bin:/bin:/usr/sbin:/sbin`;
  `lf session resume --task b2ad67` started real Claude (its trust dialog showed), provider
  `PATH=/usr/bin:/bin:/usr/sbin:/sbin:~/.local/bin:~/.opencode/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/local/sbin:~/.cargo/bin`.
- One Rust list: `os_process::TOOL_DIRS`. `LF_SUPERVISED`, `supervised_path`,
  `DESKTOP_PATH`, `REMOTE_PATH`, the open `conversation_path` and `PATH=` in
  `Attachment::argv`: no matches. Swift `enrichedPath` remains for what Swift spawns.
- Missing provider (empty `HOME`, `agent claude`), `lf open --task 38ed69 --host tmux --tab`:
  exit 1; `--json` `reason` and text both
  "'claude' is not installed. Install it with `npm install -g @anthropic-ai/claude-code`, then open `claude` to sign in; retry the same command";
  pane shows the error and "Press Enter to close"; `logs/lf.log` has a line from both sides.
- Harness: plain open's pane command is `lf session resume <id>`; `-a codex` adds
  `--agent codex` and moved the conversation to Codex (pid 58711); two more plain
  opens left pid 58711 running and `task status` provider `codex`.

## CI on 1a6c9d618a

Push `CI`: success. `macOS on main` and `Packages (main)`: in progress when read.

## Not run

Rex, Herdr, cmux and Desktop opens; a harness move replayed after the conversation ended.

## Fix forward, built 2026-11-19

`open/tmux.rs` resolves `tmux` on lf's `PATH`, then runs the `new-window`
client with no `PATH`. tmux takes a window's `PATH` from an unattached client
only when the client has one; with none the window keeps the server's.

Seen on a private tmux 3.7c server started with `PATH=/usr/bin:/bin`:
`PATH=/tmp/agentshim:$PATH tmux new-window 'echo $PATH'` printed the client's
whole `PATH`; `env -u PATH /opt/homebrew/bin/tmux new-window 'echo $PATH'`
printed `~/.cargo/bin:/usr/bin:/bin` (the server's, plus what zsh startup adds).

Test: `cargo test -p loopflow --test integration work_open_tests::tmux` (2 passed, implement).

Other hosts, read from the requests lf sends, not driven: Herdr gets a socket
message with `"env":{}`; Rex gets `rex api call` with a JSON block holding
`cwd` and `command` only; cmux gets a CLI call naming a command. None carries
lf's environment in the request. Whether the `rex` and `cmux` clients add
their own is not shown by this; a live open on each (ps -Eww of the provider
after `export PATH=/tmp/shim:$PATH`) goes to verify.
