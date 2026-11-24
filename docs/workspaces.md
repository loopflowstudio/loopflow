# Open conversations and workspaces

```bash
lf open --task ENG-123              # attach the Task's conversation in this terminal
lf open --task ENG-123 --tab        # open it in a sibling tab
lf open --task ENG-123 --focus      # reveal an existing view
lf task run ENG-123 --open          # start or continue its Graph, then open its conversation
```

`lf open` attaches to a single conversation: from a Task checkout it picks that
Task's primary Session; from a Wave or repo checkout it picks that scope's
conversation. Pass `--task`, `--wave`, or `--session <id>` to target another.
Opening creates a first conversation if none exists and continues ended
Sessions from saved history, without advancing a Graph or replaying prompts.

## Choose where it appears

| Mode | Result |
|---|---|
| `--here` | Attach in the current terminal (default for direct CLI calls, including inside Rex) |
| `--tab` | Attach in a sibling tab in the current host (default for agent tool calls) |
| `--focus` | Reveal an existing view on this Machine; creates nothing (reports **Not open** if none) |

Pass at most one mode. Agent tool calls never attach recursively inside their
own output. Multiple views of the same conversation can be open simultaneously
and every view can type; the view you used last sets terminal size and colors.
Desktop shows each conversation once and focuses its existing pane.

```bash
lf open --task ENG-123 --tab --host rex
lf open --task ENG-123 --tab --host tmux
```

`--host desktop|rex|cmux|herdr|tmux|terminal` selects the host explicitly (no
silent fallback on failure). Plain terminals support `--here`; when `--tab`
isn't supported by the host, `lf` prints the command to run in a terminal you
open yourself. `--server <endpoint>` targets a specific Rex Unix socket.

## Open a Task that runs on another Machine

```bash
lf open --task LOO-12 --tab                        # LOO-12 runs on mini; the tab opens here
lf open --task LOO-12 --session session_9f2 --tab  # one of LOO-12's Sessions, on mini
lf open --machine code-1 --session session_4ab --tab  # a Session that exists only on code-1
```

`lf open` always presents on the Machine you type it on. With `--tab`, Rex,
cmux, Herdr, and tmux open a local tab that attaches to the conversation through
this Machine's Loopflow server. `lf --machine code-1 open …` means the same as
`lf open --machine code-1 …`.

The Task names its Machine, so `--task` needs no `--machine`. A checkout of the
Task left on this Machine from before it moved is never opened in its place.
Use `--machine` for a Session with no Task here, such as one started on a
rental.

| What happens | You see |
|---|---|
| The Machine doesn't answer | The open fails naming the Machine, with `ssh <route> lf server status` to check it; retry the same command |
| The Task has no checkout yet | **Not started**; `lf task run LOO-12` starts it |
| The Task moves while its tab is open | The tab stays on the Session it opened, on the old Machine, until that Session ends; `lf open --task LOO-12` again opens it where it is now |

## Open a workspace overview

```bash
lf open --wave product --workspace  # open every started, unfinished Task in `product`
lf open --workspace                 # open every started, unfinished Task in this repo
lf open --available                 # open conversations ready for your next turn
lf diff ENG-123 --base parent --difftool  # inspect one Task's changes in your diff tool
lf task file ENG-123 README.md --at parent  # read a file as it was before the Task
```

`--workspace` opens an overview of started, unfinished Tasks: Desktop opens its
backlog, while Rex, Herdr, and cmux open native Task tabs (one per Task
conversation, in a new Rex session, Herdr workspace, or cmux group). Pass
`--no-focus` to keep your current tab selected.

External hosts own their own layouts: add splits, shells, and editors with
your terminal's normal controls. Desktop remembers your per-Task pane layout on
each Mac independently.

## Split in the right directory

Use your terminal's normal Split shortcut from an open Conversation tab (or
after `lf resume`): the new shell starts in that Task's worktree. From a shell
tab, splits follow that shell's current `cd` directory. After detaching with
`Ctrl+]`, splits return to the parent shell's directory.

## Failure and worktree cleanup

- **Attachment errors.** Success means the Session attached and its provider
  started. If not, the command fails with lf's own error, and the pane shows the
  same text and waits for Enter. Fix it and retry (for a lost reply where
  attachment may have succeeded, use `--focus` first):

  ```bash
  lf open --task ENG-123 --tab --host rex --json
  # {"receipts": [{"host": "rex", "status": "failed",
  #   "reason": "'claude' is not installed. Install it with `npm install -g @anthropic-ai/claude-code`, then open `claude` to sign in; retry the same command", ...}]}
  ```
- **Choosing a harness.** If multiple harnesses are installed with no default,
  pass `-a claude` or save `lf set --global default_agent claude`. Without `-a`,
  a pane continues on the harness the conversation last ran on;
  `lf -a codex open --task ENG-123` moves it
  ([switching agents](sessions.md#switch-agents-mid-conversation)).
- **Finding the harness.** A pane looks for the harness after the host's own
  `PATH` in `~/.local/bin`, `~/.opencode/bin`, `/opt/homebrew/bin`,
  `/opt/homebrew/sbin`, `/usr/local/bin`, `/usr/local/sbin` and `~/.cargo/bin`.
  It does not inherit your terminal's `PATH`, so a tool reachable only through
  shell startup files (nvm, mise, asdf or pyenv shims) is missing there unless
  it is also in one of those directories.
- **Completed Task worktrees.** Keeping a Conversation pane open preserves a
  completed Task's worktree. Once the Task is done and no Conversation pane is
  open, Loopflow deletes the merged worktree (and any uncommitted files in it)
  after ~15 minutes. Task and conversation history always remain.

Next: [Surfaces](surfaces.md) · [Connect clients](server.md).
