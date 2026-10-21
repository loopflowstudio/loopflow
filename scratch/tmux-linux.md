# tmux on Linux, as good as cmux

Jack Heart wants Loopflow to feel deliberate inside tmux on Linux for the next
person he shows it to (a tmux user, likely on Windows later; Linux first).
"Great" is the bar the cmux/Herdr trials set (LOO-421/422/423/426/530):

- No hook errors: no hook stderr, no Stop-hook "keep working" loops, nothing
  printed into the conversation by Loopflow plumbing, on a real Claude Session.
- No environment leakage or dropping: a conversation's host context follows
  where it is viewed now, not the pane it was launched from.
- Links open in the natural place: `lf open` inside tmux (by a person or an
  agent) opens a new window right after the current one, named for the Work,
  focused once; reopening focuses the existing window.

## Built (proven headless on a rented Ubuntu 24.04 / tmux 3.4 box)

- `--host tmux` and the automatic route: Herdr, then tmux, then cmux. A new
  window `-a` after the caller's, `-n` the Work, destination and placement the
  pane id (`tmux:%N`). A vanished caller pane falls back to the server's
  current session. Focus is select-window/select-pane, then switch-client.
- Reopen after the window closed: `lf open` asks the transport whether a
  viewer holds control (`Request::Controlled`) before reporting `elsewhere`;
  a running conversation with no viewer opens in the new window.
- A viewer taking control records its own placement on the Session, and an
  agent's `lf open` routes by that placement before its inherited
  `TMUX`/`CMUX_*`/`HERDR_*`.
- Window names: the viewer, not the process inside the transport, names its
  tmux window (or cmux workspace) for the Session, follows renames, and
  restores tmux automatic-rename when it leaves.

Hooks on a real signed-in Claude Session (Claude Code 2.1.306) in tmux on
Linux: an interactive turn with a tool call and a failed tool, detach, `lf
resume`, another turn, then `lf -b`: no hook error on screen, empty
`hookErrors` in every transcript record, clean headless stderr.

Jack Heart chose less debug output in production, broadly: tracing goes to
`logs/lf.log` (`-v`/`RUST_LOG` for stderr), and launches no longer announce
diagnostics paths (Loopflow's or OpenCode's). LOO-554 carries the four-harness
QA of this on Linux.

## Left

- Linux Worktree sandbox (`failIfUnavailable`, bubblewrap/socat) and the two
  `--settings` flags (hooks + sandbox) under a real Session.
- A conversation viewed from a plain terminal records no placement, so its
  agent still falls back to the inherited tmux pane's session.
- Installed binaries run as `lf-<sha>`; tmux shows that for lf commands
  that are not Sessions. argv[0] feeds Process records, so not changed here.
- Clicked links: tmux passes OSC 8 to the outer terminal, whose handler runs
  outside tmux. Windows/WSL follow-up.

## Questions


- Herdr inside tmux vs tmux inside Herdr both carry both env sets; this picks
  Herdr. Revisit if someone nests them.
