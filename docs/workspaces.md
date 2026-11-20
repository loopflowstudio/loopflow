# Open conversations and workspaces

```bash
lf open --task ENG-123              # attach the Task's conversation in this terminal
lf open --task ENG-123 --tab         # open it in a sibling tab
lf open --task ENG-123 --focus       # reveal its existing view
lf task run ENG-123 --open          # start or continue its Graph, then open its conversation
```

`lf open` opens one conversation. From a Task checkout it selects that Task's
primary Session; from a Wave or repository checkout it selects that scope's
conversation. Use `--task`, `--wave` or `--session <id>` to choose another.
A Task without a conversation gets its first one. An ended Session continues
from its saved history. Opening neither advances a Graph nor replays a prompt.
To run the Graph and open the conversation together, use
[`lf task run --open`](work.md#hand-off-more-than-one).

## Choose where it appears

| Mode | Result |
| --- | --- |
| `--here` | Attach in the current terminal; the default for a direct CLI invocation, including inside Rex. |
| `--tab` | Attach in a sibling tab in the selected host; the default for an agent tool call. |
| `--focus` | Reveal an existing view of this conversation; create nothing. |

Choose at most one mode. Desktop's normal navigation reveals its existing view.
A terminal invocation does not switch to Desktop just because it is installed.
Agent calls never attach recursively inside their tool output, including when
opening their own Session. An agent's `--here` request fails with a command to
run from an interactive terminal; it does not change the requested mode.

A conversation takes any number of views, and every one types; a draft typed
in one shows in the others. If it is already open, `--here` and `--tab` attach
one more view beside the existing ones. Desktop shows a conversation once:
opening it there focuses its existing pane. The view used last (attached, typed
in or focused) sets the terminal's size and colors; when it closes, the one
used before it takes over.

`--focus` reveals an existing view and creates nothing. Detection covers known
live views across local hosts on this Machine and matches the Session and view
IDs, even if tabs were renamed. It does not search other Machines. A view may
still be opening or show an attachment error; `--focus` only reveals it. After
correcting an attachment failure, repeat the opening command.

A conversation whose provider runs in a client lf cannot share, such as one
started in a plain terminal before, is taken over: that client ends and the
conversation continues in the new view.

`--focus` reports **Not open** when no view exists, including while Desktop is
still resolving an opening before creating its pane. A view the host cannot
focus is reported with its location and the failure. Neither case launches a
replacement. To start a closed conversation, run the command again without
`--focus`.

```bash
lf open --task ENG-123 --tab --host rex
lf open --task ENG-123 --tab --host tmux
```

`--host desktop|rex|cmux|herdr|tmux|terminal` chooses the destination. Without it,
`--tab` uses the calling terminal's host, or the agent conversation's host.
`--here` uses the current terminal. An incompatible host and mode reports an
error. LF never changes hosts after a failure.

A plain terminal supports `--here`. With no sibling-tab support, `--tab` fails
and prints a command to run in a terminal you open yourself. The same applies
to an agent call without a supported host; it does not fall back to attaching
inside the tool call. Use `--host desktop` to choose Desktop navigation.
`--server <endpoint>` selects a Rex Unix socket. Rex workspaces can
reach a Task's recorded Machine; Herdr and cmux workspaces stay local.

## Open a workspace

```bash
lf open --wave product --workspace  # Product's started, unfinished Tasks
lf open --workspace                 # this repository's started, unfinished Tasks
lf open --available                 # conversations ready for your next turn
lf diff ENG-123                     # only its changes
```

Without `--workspace`, `lf open --wave product` attaches the Wave's
conversation. With it, a repository or Wave opens as an overview of its
started, unfinished Tasks, once: a Task started later opens with
`lf open --task <task>`. A Task has no overview of its own, so `--workspace`
with a Task names the Wave or repository overview to open instead.
Desktop shows its board, while Rex, Herdr and cmux arrange native Task tabs.
Each Task tab is the Task's conversation: an ended one starts its provider
when the tab opens, so an overview of twelve Tasks runs twelve agents.

An overview gets a new Rex session, Herdr workspace or cmux group. A single
conversation's tab (`--tab`) joins the current one; cmux uses the current group,
or the window if there is no group. External hosts own these arrangements: use their controls to
rearrange, rename, close and restore them. `--host` selects a host;
`--no-focus` keeps the current tab selected. Hosts without workspace support
report that limitation; drop `--workspace`, or use `lf diff`, for a single
activity.

## What a Task tab holds

Each Task tab in an overview holds the Task's Session, opened as
`lf open --task ENG-123 --here` would. Every view of a Session can type into
it, and you see drafts from one view in the others.

Add shells, editors or splits with the host's own controls; LF builds no layout
in Rex, Herdr or cmux, and nothing it does there changes Desktop. Desktop
remembers how you arranged each Task on that Mac, and only there: a second Mac
driving the same Machine keeps its own arrangement. Unsent file drafts and
background tabs remain in Desktop.

Opening a conversation does not start the Task's Graph or replay its prompt.

```bash
lf diff ENG-123 --base parent --difftool    # the Task's changes in your diff tool
```

## Split in the directory you are using

Use your terminal app's normal Split shortcut. From a Conversation tab, the new
shell starts in the Task worktree. From a shell, it follows that shell's
current directory, including changes made with `cd`.

```bash
pwd -P                              # check the new shell's directory
```

This also applies when you run `lf resume` manually in another terminal. After
leaving the conversation with Ctrl+], the parent shell's directory governs new
splits again.

## If opening fails

```bash
lf open --task ENG-123 --tab --host rex --json
# {"status": "failed", "reason": "'claude' is not installed. Install it with `npm install -g @anthropic-ai/claude-code`, then open `claude` to sign in; retry the same command", ...}
```

A successful conversation open means the Session attached, not just that the
host created a tab. When it doesn't attach, the opening command fails with the
reason the conversation gave: lf's own error, or the provider's last words if it
exited before it was ready. The pane keeps the same text on screen and waits
for Enter. Correct the cause and retry the same command. Desktop rechecks its
pane before opening another; a terminal retry attaches one more view, so if attachment completed before the
reply was lost, `--focus` reveals the first one.

Starting a Task's first conversation needs a harness. With no `default_agent` and
several harnesses installed, choose one for this invocation or set a default:

```bash
lf -a claude open --task ENG-123
lf set --global default_agent claude
```

Opening never changes a conversation's harness on its own. Without `-a`, the
pane continues on the harness the conversation last ran on, so a retry, a
restored tab or Desktop reopening it lands where `lf -a codex resume` last put
it. `lf -a codex open --task ENG-123` moves it, as
[switching agents](sessions.md#switch-agents-mid-conversation) does.

A pane finds the harness and its tools wherever the host was started, including
a Rex or tmux server started at login: lf looks in `~/.local/bin`,
`/opt/homebrew/bin`, `/opt/homebrew/sbin`, `/usr/local/bin`, `/usr/local/sbin`
and `~/.cargo/bin` after the host's own `PATH`. The pane does not inherit the
`PATH` of the terminal you ran `lf open` from, so a tool reachable only through
your shell startup files (nvm, mise, asdf or pyenv shims) is missing there
unless it is also in one of those directories.

A host or attachment failure never substitutes a workflow pane for the
conversation. In a workspace, tools that opened successfully remain available
when another part fails; the command reports which part failed.

Use your terminal app to restore its tabs after a restart. Restoring a layout may
start fresh shells; it does not necessarily preserve running programs. Closing a
view does not delete the Task. LF closes only its Desktop-owned shells; external
terminal apps own theirs.

## Keep a completed Task's worktree

A Conversation pane keeps a completed Task's worktree, focused or not. Task &
workflow and shells do not, so a tab without a Conversation can lose its directory
while you use it. Run `lf open --task <task>` to open a Conversation before you
finish work there.

With no Conversation pane open, LF deletes a merged Task's worktree and any
uncommitted files in it about 15 minutes after the Task completes or the last
Conversation pane closes, whichever is later. A conversation whose process stops
responding counts as closed about 30 seconds after its last report. Opening a
Conversation cancels the countdown; closing it starts a new one. Requested work
finishes first. Task and conversation history remain.
