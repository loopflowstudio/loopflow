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
open yourself. `--server <endpoint>` targets a specific Rex Unix socket. Rex
workspaces can reach a Task's remote Machine; Herdr and cmux stay local.

## Open a workspace overview

```bash
lf open --wave product --workspace  # open every started, unfinished Task in `product`
lf open --workspace                 # open every started, unfinished Task in this repo
lf open --available                 # open conversations ready for your next turn
lf diff ENG-123 --base parent --difftool  # inspect one Task's changes in your diff tool
```

`--workspace` opens an overview of started, unfinished Tasks: Desktop opens its
board, while Rex, Herdr, and cmux open native Task tabs (one per Task
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

- **Attachment errors.** Success means the Session actually attached. If a host
  or attachment fails, fix the reported error and retry (for a lost reply where
  attachment may have succeeded, use `--focus` first).
- **Choosing a harness.** If multiple harnesses are installed with no default,
  pass `-a claude` or save `lf set --global default_agent claude`.
- **Completed Task worktrees.** Keeping a Conversation pane open preserves a
  completed Task's worktree. Once the Task is done and no Conversation pane is
  open, Loopflow deletes the merged worktree (and any uncommitted files in it)
  after ~15 minutes. Task and conversation history always remain.

Next: [Surfaces](surfaces.md) · [Connect clients](server.md).
