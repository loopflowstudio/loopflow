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
backlog, while Rex, Herdr, and cmux open native Task tabs (one per Task
conversation, in a new Rex session, Herdr workspace, or cmux group). Pass
`--no-focus` to keep your current tab selected.

External hosts own their own layouts: add splits, shells, and editors with
your terminal's normal controls. Desktop remembers your per-Task pane layout on
each Mac independently.

## Search and read a Task's checkout

Every checkout read runs on the Machine that owns the Task, so local and remote
Tasks answer the same commands:

```bash
lf task find ENG-123 TaskFile                    # fuzzy-find paths across tracked, untracked, and scratch/ files
lf task grep ENG-123 ReviewSelection             # literal content search (--whole-word for identifier references)
lf task symbols ENG-123 reconcile                # find declarations across the checkout (--path narrows to one file)
lf task file ENG-123 README.md --at parent       # read a file as it was before the Task
```

```text
$ lf task grep ENG-123 ReviewSelection
swift/LoopflowMac/ReviewDiff.swift:14:struct ReviewSelection: Equatable {
swift/LoopflowMac/Views/ReviewStreamView.swift:82:    var selection: ReviewSelection?

$ lf task symbols ENG-123 reconcile
swift/LoopflowMac/TaskFileDocument.swift:318:func reconcile (TaskFileDocument)
```

- `lf task find <ID> [query]` returns tracked, nonignored untracked, and
  `scratch/` paths (up to 100 ranked matches for a query, or the full inventory
  when no query is given and the checkout fits within 2,000 paths). Add `--json`
  for `scratch` paths, `latest_scratch_page`, and `truncated`. Use `--limit N`
  to set how many paths a reply contains. A query searches the full inventory
  even when the initial list was truncated.
- `lf task grep <ID> <query>` searches the same inventory (literal by default;
  pass `--whole-word` for whole-word references), capping at 100 hits and
  skipping files over 1 MB. `--json` includes 1-based line numbers, UTF-8 byte
  spans, excerpts, and `truncated`. Each occurrence has its own hit.
  `start` is inclusive and `end` is exclusive, measured from the start of the
  line in the file’s reported `revision`.
- `lf task symbols <ID> <name> [--path <path>]` finds declarations (functions,
  types, methods, and Markdown headings) with their enclosing container, path,
  and line number, capping at 100 hits.

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
