# LOO-986 verify, pass 2, 2026-11-19

Main at `f7db10af76` (#1994; this checkout's tree is identical), `cargo build -p loopflow --bin lf`,
private `LF_HOME` and repo under `/tmp/v986c`, own `lf server run` and tmux 3.7c
(`tmux -L v986c -f /dev/null`), all started with
`env -i HOME USER TERM LF_HOME PATH=/usr/bin:/bin:/usr/sbin:/sbin`.

Outcome: **verified**. The claim pass 1 found failing now holds.

## An agent's PATH no longer reaches the conversation it opens into tmux

    pane %0$ export PATH=/tmp/v986c/agentshim:$PATH
    pane %0$ lf open --task 8cd6e4 --host tmux --tab --json   -> exit 0, status usable, destination %1
    pane %1 command: … env -u LF_AGENT_CALLER -u LF_AGENT_OVERRIDE LF_HOME=… LF_OPEN_RECEIPT=… lf session resume <id>   (no PATH=, no --agent)
    pane %1 screen: real Claude's trust dialog for /private/tmp/v986c/repo
    ps -Eww PATH:
      claude --name 8cd6e4 …   /usr/bin:/bin:/usr/sbin:/sbin:~/.local/bin:~/.opencode/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/local/sbin:~/.cargo/bin
      lf holder run            same
      lf session attach        ~/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:~/.local/bin:… (no agentshim)
    control, same pane: tmux new-window -d 'echo $PATH > control'  -> /tmp/v986c/agentshim:~/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin

## Carried from pass 1 (main at 1a6c9d618a, code untouched by #1994)

launchd-PATH resume, the single `os_process::TOOL_DIRS` list, the missing-provider
reason in text and `--json`, and the harness staying put all held.

## CI on f7db10af76

Push `CI`: success. Merge-group `CI`: success. `macOS on main` and `Packages (main)`: in progress when read.
On 1a6c9d618a: push `CI`, `macOS on main`, `Packages (main)` all success.

## Not run

Rex, Herdr, cmux and Desktop opens on main (the branch's qa step drove Rex and Herdr before merge).
