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

If the conversation is already open, `--here`, `--tab` and ordinary terminal
opening report where it is and suggest `--focus`. They neither take its input
nor create another view. Detection covers known live views across local hosts
on this Machine and matches the Session and view IDs, even if tabs were renamed.
It does not search other Machines. A known remote viewer is reported with its
location; opening does not take it over. To deliberately move input, use
`lf resume <session>`.

A view may still be opening or show an attachment error. `--focus` only reveals
it. After correcting an attachment failure, repeat the opening command to
reconnect; an attachment still in progress is reported without launching another.

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
`--server <endpoint>` selects a Rex Unix socket. Rex workspace layouts can
reach a Task's recorded Machine; Herdr and cmux workspace layouts stay local.

## Open a workspace

```bash
lf workspace --task ENG-123         # the Task's configured panes in your host
lf workspace --wave product        # Product's started, unfinished Tasks
lf workspace                       # this checkout's Task, Wave or repository
lf workspace --available           # conversations ready for your next turn
lf task view ENG-123                # only the Task brief and workflow
lf diff ENG-123                     # only its changes
```

`lf workspace` opens arrangements and overviews. In a repository or Wave, it
shows started, unfinished Tasks; backlog Tasks appear only when selected.
Desktop shows its board, while Rex, Herdr and cmux arrange native Task tabs.
Use `lf open --task <task>` to enter a conversation from an overview. Overviews
and automatic arrivals start no agents and leave existing conversation input
where it is. A Task's configured workspace can hide Conversation; ordinary
`lf open` always reaches the requested conversation regardless of that layout.

Rex and Herdr place a Task in the current session or workspace; cmux uses the
current group, or the window if there is no group. A repository or Wave gets a
new container. External hosts own these arrangements: use their controls to
rearrange, rename, close and restore them. `--host` selects a host;
`--no-focus` keeps the current tab selected. Hosts without workspace support
report that limitation; use `lf open`, `lf task view` or `lf diff` for a single
activity.

## Choose a layout

`lf workspace` uses the first available layout:

1. The Task's saved Desktop arrangement.
2. Its `task_layout` configuration, including Wave preferences.
3. The builtin Conversation, Task & workflow and Shell layout.

Only visible panes and their selected tabs open. LF recreates the arrangement as
closely as the host supports and reports tools it cannot display. A Conversation
pane uses the same Session resolution and attachment rules as `lf open`. If it
is already open elsewhere, the workspace reports its location without moving
input or substituting Task & workflow for the requested conversation.

Arrange a Task in Desktop to save its layout for later launches. Changes made in
Rex, Herdr or cmux stay in that app; they do not change Desktop's saved layout or
your templates. Opening another workspace creates another arrangement, without
duplicating existing conversation views. Unsent file drafts and background tabs
remain in Desktop.

## Set a Wave's default

This template opens Changes beside a shell and hides Conversation:

```yaml
# .lf/wave/product/config.yaml
task_layout:
  hidden: [conversation]          # pane IDs to leave out
  tree:
    kind: split
    axis: vertical
    ratio: 0.6
    first:
      kind: pane
      id: changes
      content:
        kind: browser
        location: null
        tabs: [{kind: changes, id: changes, comparison: {kind: working_tree, base: parent}}]
        selected: changes
    second:
      kind: split
      axis: horizontal
      ratio: 0.5
      first: {kind: pane, id: conversation, content: {kind: session, session_id: null, location: null}}
      second:
        kind: pane
        id: shell
        content: {kind: shell, tabs: [{id: s, location: null, lf_process_id: null}], selected: s}
```

Include a conversation, shell and browser pane; add unused pane IDs to `hidden`.
Leave Session IDs and locations unset so they resolve from the Task when opened.
Templates cannot contain open file drafts or specific conversations.

Use these configuration files:

- `.lf/wave/<name>/config.yaml` for a shared Wave default.
- `lf.config.local.yaml` for private repository and Wave overrides.
- `.lf/config.yaml` or `~/.lf/config.yaml` for broader defaults.

See [configuration scopes](config.md#where-settings-live). Task worktrees share
the repository's private configuration. The winning scope replaces the whole
layout. A saved Desktop Task arrangement takes precedence over a template.
Changing a template affects future launches; existing tabs stay as you arranged them.

## Choose your tools

| Pane | What opens in a terminal host |
| --- | --- |
| Conversation | The Task's primary Session, subject to the same already-open rules as `lf open` |
| Shell | A fresh shell in the Task worktree |
| Files | Your file browser or editor |
| Changes | Git's pager or your diff tool |
| Task & workflow | The Task brief, workflow and recorded runs |

Opening a Conversation pane does not start the Task's Graph or replay its
prompt. A new shell starts in the worktree without copying another shell's jobs
or variables.

```bash
export LF_FILE_BROWSER=yazi          # directory browser
export EDITOR=vim                    # file editor
lf task view ENG-123                 # inspect the Task and workflow
```

Files uses `LF_FILE_BROWSER` for directories, then `VISUAL`, `EDITOR`, or `vi` for
editing. Set these on the Machine where the Task runs. GUI editors may open their
own windows.

Changes runs `lf task diff`, using Git's pager and diff settings. Working-tree
patches include untracked files; `--difftool` covers tracked files:

```bash
lf diff ENG-123 --base parent --difftool
```

Use `--head <commit>` for a committed comparison. In Task & workflow, `n` and `p`
select a step and `l` reads its log. Viewing starts no work.

## Follow newly started Tasks

A repository or Wave opened with `lf workspace` in Rex includes a Workspace tab.
Keep it running to add newly started Tasks without taking focus or adding backlog
Tasks. A Task tab you close stays closed while that Workspace tab runs.

New arrivals show Task & workflow information and the configured tools without
attaching conversations. They leave input in your existing terminals. Run
`lf open --task <task>` when you want to resume that conversation.

Following includes remote Tasks on their recorded Machines. Starting a Task does
not reopen a closed host app. Single-Task opens do not follow sibling Tasks;
Herdr and cmux do not support following.

## Split in the directory you are using

Use your terminal app's normal Split shortcut. From a Conversation or Task tool,
the new shell starts in the Task worktree. From a shell, it follows that shell's
current directory, including changes made with `cd`.

```bash
pwd -P                              # check the new shell's directory
```

This also applies when you run `lf resume` manually in another terminal. After
leaving the conversation with Ctrl+], the parent shell's directory governs new
splits again.

## If opening fails

A successful conversation open means the Session attached, not just that the
host created a tab. Attachment failures return a failure to the opening command
and leave readable diagnostics in the destination. Correct the reported cause
and retry the same command; LF rechecks the Session's live view rather than
blindly adding another conversation tab. If attachment completed before the
reply was lost, the retry reports its location and offers `--focus`.

Starting a Task's first conversation needs a harness. With no `default_agent` and
several harnesses installed, choose one for this invocation or set a default:

```bash
lf -a claude open --task ENG-123
lf set --global default_agent claude
```

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
