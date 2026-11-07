# Open Task workspaces

```bash
lf open                             # open this Task or repository in your current host
lf open --task ENG-123 --host rex    # open a Task in Rex
lf open --wave product              # open Product's started, unfinished Tasks
```

In Rex, Herdr or cmux, each open creates a fresh Task tab with its configured
splits. Conversation panes run `lf resume` in the Task's worktree. Return to existing tabs through your
terminal app; use its controls to rearrange, rename, close and restore them.

Opening a Task starts its conversation. Opening a repository or Wave starts no
agents: it reconnects conversations that are still running and shows Task & workflow
for the rest. Run `lf open --task <task>` to pick one up. A conversation running
the command stays where it is. A conversation with no Task opens by itself:
`lf open --session <id>` adds one Conversation tab where you are, and
`lf open --available` does the same for each such conversation.

From a Task checkout, `lf open` selects that Task. From a repository checkout, it
selects the repository's started, unfinished Tasks. Use `--task` or `--wave` to
choose another scope. Backlog Tasks appear only when explicitly selected.

## Choose a host

```bash
lf open --host herdr
lf open --host cmux
lf open --host rex --no-focus        # keep the current tab selected
```

Without `--host`, LF uses the terminal you run it from. Commands issued by an agent
use the host of that conversation's current terminal. Outside a recognized host,
LF opens Desktop on macOS, then falls back to a plain terminal if needed.

| What you open | Rex | Herdr | cmux |
| --- | --- | --- | --- |
| Repository or Wave | Session | Workspace | Workspace group |
| Task | Tab | Tab | Workspace |
| Task layout | Splits | Panes | Split panes |

A Task opened inside Rex or Herdr joins the current session or workspace. In cmux,
it joins the current workspace's group, or the window when there is no group.
Opening a repository or Wave creates a new container named for it.

Rex, Herdr and cmux support layouts. tmux and plain terminals open the conversation
alone. Herdr and cmux support local Tasks only. Rex can open remote Tasks on their
recorded Machine. Use `--server <endpoint>` to select a Rex Unix socket.

If the selected host is unavailable, LF reports the error instead of opening
somewhere else. `--no-focus` preserves the selected tab; a Conversation pane still
runs normal resume, which moves conversation input to the new terminal. That
includes a running conversation reconnected by a repository or Wave open.

## Choose a layout

`lf open` uses the first available layout:

1. The Task's saved Desktop arrangement.
2. Its `task_layout` configuration, including Wave preferences.
3. The builtin Conversation, Task & workflow and Shell layout.

Only visible panes and their selected tabs open. LF recreates the arrangement as
closely as the host supports and reports tools it cannot display.

Arrange a Task in Desktop to save its layout for later launches. Changes made in
Rex, Herdr or cmux stay in that app; they do not change Desktop's saved layout or
your templates. Opening again creates another tab from the current LF layout.
Unsent file drafts and background tabs remain in Desktop.

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
| Conversation | `lf resume <session>` in the Task worktree, or Task & workflow when a repository or Wave open finds no running conversation |
| Shell | A fresh shell in the Task worktree |
| Files | Your file browser or editor |
| Changes | Git's pager or your diff tool |
| Task & workflow | The Task brief, workflow and recorded runs |

Conversation panes use ordinary resume. If the Task has no conversation, they run
`lf resume --task <task>` to start one, and that new conversation takes its first
turn on the Task. Opening does not start the Task's Graph. A new shell starts in
the worktree without copying another shell's jobs or variables.

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

A repository or Wave opened in Rex includes a Workspace tab. Keep it running to
add newly started Tasks without taking focus or adding backlog Tasks. A Task tab
you close stays closed while that Workspace tab runs.

New arrivals use the configured splits with Task & workflow information in the
Conversation slot. They leave input in your existing terminals. Run
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

Check the error in the terminal. A successful open receipt means the native tab
was created; resume may still be connecting or may report its own error there.

Starting a Task's first conversation needs a harness. With no `default_agent` and
several harnesses installed, the open fails and creates no tab. Choose one for
this Task, or for every Task:

```bash
lf -a claude open --task <task>
lf set --global default_agent claude
```

If LF reports that a tab may have opened without a reply, look for it in your
terminal app before running the command again. Each invocation creates fresh
tabs. Successfully opened tabs remain available when another part of the launch
fails.

Use your terminal app to restore its tabs after a restart. Restoring a layout may
start fresh shells; it does not necessarily preserve running programs. Closing a
view does not delete the Task. LF closes only its Desktop-owned shells; external
terminal apps own theirs.
