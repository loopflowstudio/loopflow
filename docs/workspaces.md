# Launch your Task layout in a terminal host

> Accepted design, revised November 5, 2026. Fresh launch, config templates and the
> Rex, Herdr and cmux renderers are implemented; gaps are marked where they apply.

```bash
lf open --host rex --task LOO-498     # launch this Task's layout in Rex
lf open --host rex --wave product    # launch Product's started, unfinished Tasks
lf open                             # select Work and host from the current context
```

For a Task, each `lf open` launches a **fresh tab with its workspace of splits**.
Use the current or explicitly selected Rex session as its container. Fresh Task
opening need not create another session. Return to an existing tab through Rex;
rename, rearrange, close and restore it using Rex's own controls. Running the
command again creates another Task tab from LF's current source layout; it does
not search for, repair or change the earlier tab.

Container choice comes from where you are, never from a stored mapping. Inside a
Rex session or Herdr workspace, a Task tab joins it. In cmux, a Task workspace
joins the group your workspace is in now, or your window when it has no group.
Outside one, or when
opening a repository or Wave, `lf open` creates a new container named for the
Wave or repository.

Opening presents existing Work. It does not start a Task's Graph, create a provider
conversation, submit a turn or take conversation input. A Shell tool creates a
fresh shell; it does not copy jobs, shell variables or editor memory.

From a repository checkout, select its started, unfinished Tasks in one container.
From a Task checkout, select that Task. Use `--wave` or `--task` explicitly to
select another scope. Backlog Tasks appear only when
explicitly selected; opening them starts no execution.

Inside Rex, resolve the current host/server and caller context, including when an
agent invokes `lf open` through a tool shell. `--host` overrides automatic host
selection, `--server` selects a Rex Unix endpoint, and `--no-focus` preserves
selection. A remote Task stays on its recorded Machine; failure must not create
local replacement shells. Fresh launch does not mean another host or Machine.

## Use the host you are in

`lf open` detects the current host for direct callers and agent tools. `--host` overrides that choice. Resolve
an agent's current attached viewer rather than its stale launch environment. In
nested terminals, use the actual inner host. A host that is merely installed or
running is not evidence that the caller belongs to it.

Map LF's concepts to the host's own structure:

| LF concept | Rex | Herdr | cmux macOS |
| --- | --- | --- | --- |
| Repo or Wave container | Session | Workspace | Workspace group |
| Task | Tab | Tab | Workspace/sidebar item |
| Task workspace | Splits inside the tab | Panes inside the tab | Split panes inside the workspace |

Herdr's workspace/tab/pane API maps directly to the hierarchy.
[Herdr concepts](https://herdr.dev/tui/docs/concepts/)

cmux's Task-level object is a workspace; its per-pane surface tabs are a level
below the Task. Groups provide the outer container.
[cmux concepts](https://cmux.com/docs/concepts),
[workspace groups](https://cmux.com/docs/workspace-groups)

An explicit native window/container can also supply the outer grouping. The
adapter creates native resources and reports capability differences. It does
not persist their identities or copy their state into LF. Failure to reach the
selected host never sends work to another host: inside Rex, Herdr, tmux or cmux,
that host is the only route. Without current-host context, Desktop then a plain
terminal, or an explicit `--host`. Herdr and cmux open on this Machine only; a
remote Task fails rather than opening locally. cmux opens through its bundled CLI
when `cmux` is not on PATH; its renderer has fixture coverage, not native proof.

## Templates, current layouts, and native workspaces

| What you change | Where it belongs | Effect |
| --- | --- | --- |
| Default tools and arrangement for new Task workspaces | LF configuration | Used on later launches when the Task has no saved layout |
| This Task's current arrangement in Desktop | LF database | Sticky Desktop layout; readable as input for a terminal launch |
| Splits, tabs, shell state and restoration in Rex | Rex | Changes that native workspace only |

For each Task, choose the launch input in this order:

1. Its current saved Loopflow Desktop layout.
2. Its resolved configuration template, including Wave preferences.
3. The builtin layout.

Read the selected input and ask the host to reproduce it **as faithfully as
possible**. Only visible, selected tools materialize. Reading a Desktop layout for
Rex does not modify it. Rex edits never write back into LF. A selected tool that
cannot be represented is reported, not silently executed as something else.

LF stores no external-host workspace registry, native destination IDs, named-copy
records, creation journal or mirror of native edits. This applies to files and
sidecars as well as SQLite. Live commands can hold their native target while
running; they do not persist a host workspace for later reopening.

Templates describe tools and arrangement, not concrete Sessions, process IDs,
execution locations, file drafts or terminal contents. Resolve those from the
selected Task when launching. Existing file drafts and background tabs stay in
Desktop's saved state; they are not copied into native shell commands.

## Configure a Wave's default

Set `task_layout` in the existing configuration scopes:

- `.lf/wave/<name>/config.yaml` for a shared Wave template.
- `lf.config.local.yaml` for private repository and Wave overrides.
- `.lf/config.yaml` and `~/.lf/config.yaml` for broader defaults.

Task worktrees share the repository's private configuration, using the existing
Wave identity and precedence rules. See [Configuration](config.md#where-settings-live).
There is no separate host-layout file or database preference layer.
`lf set task_layout` and configuration reads reject a template that a launch
could not use, such as one that captures a Session.

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

The tree uses Desktop's pane shape: one conversation, one shell and one browser
pane, each hideable. A template that names a Session, location or open file is
rejected when a launch reads it.

Changing a template affects future launches that use it. It neither rearranges
Desktop's saved Task layout nor modifies existing Rex workspaces. A saved Task
layout continues to take precedence. The winning configuration scope supplies
the whole layout; split trees are never merged across scopes.

## One Task tab, shared tools

In Rex, a repository or Wave maps to a native session, depending on what you open.
A Task maps to a tab. Its **workspace** is the set of tool splits inside that tab.
The host session is a container; LF does not store a separate workspace entity
for it. Existing Wave conversation inclusion remains part of Wave opening.

| Tool | Desktop | Terminal host |
| --- | --- | --- |
| Conversation | Terminal showing the LF conversation | Terminal observing that existing conversation |
| Shell | Terminal in the Task checkout | Fresh shell in the Task checkout |
| Files | Native file view | Configured file browser or editor |
| Changes | Native changes view | Git pager or diff tool |
| Task & workflow | Native Task view | LF terminal Task view |

```bash
lf task view LOO-498
```

In the Task view, `n` and `p` select a step and `l` reads its log. Viewing starts
no work. The builtin layout shows Task & workflow beside a shell when there is
no conversation.

For terminal file editing, use `LF_FILE_BROWSER` for a directory browser;
otherwise `VISUAL`, then `EDITOR`, then `vi`. Resolve preferences on the Task's
execution Machine. GUI editors can open their own windows.

```bash
lf task tool LOO-498 files --checkout /recorded/checkout
lf task tool LOO-498 changes --checkout /recorded/checkout --base parent --difftool
```

Changes uses Git's pager and comparison settings. Normal working-tree patches
include untracked files; explicit difftool covers tracked files. Branch and PR
comparisons retain their resolved base/head rather than mixing in uncommitted
changes. `--head <sha>` selects the committed comparison explicitly.

## New Tasks arrive without taking focus

In Rex, a repository or Wave launch includes a Workspace tab that adds newly
started Tasks without taking focus, starting another provider or adding backlog
Tasks. A single-Task launch does not follow sibling Tasks. Herdr and cmux launches
do not follow yet.

**Implementation choice, still open for review:** new Tasks arrive as normal Task
tabs with their configured splits. There is no preview-and-expand step. Ordinary
tools selected by the template start; another provider does not. The accepted
repository/Wave → session, Task → tab, workspace → splits hierarchy is explicit;
immediate full tool materialization is the stated interpretation of it.

The follower keeps its session and seen Tasks in memory, from its own command
line, while it runs. Closing a Task tab stays respected during that live follower. The host owns
its native tabs and process restoration. LF does not keep a registry to repair or
reconstruct that workspace after the follower exits.

Another `lf open --wave product` launches a new session with fresh Task tabs for
the current started, unfinished set. Starting a Task does not open a closed host
app. One Workspace tab follows every Machine the launch reaches. Other Machines'
Tasks arrive in a session on their own Machine. If the launch opened nothing on
that Machine, its first arrival creates the session, and later arrivals join it.
If that creation's reply is lost, nothing more is created there until you open
again.

## Split where you are working

Use the host's normal Split shortcut or menu. From a Conversation, Files,
Changes or Task view, the new shell should start at that Task's checkout on its
execution Machine. Tasks A and B in one Wave have their own directories.

The same applies when you manually start or resume LF in an existing terminal.
While attached, LF runs in the selected Task directory and reports it to the host.
Leaving with Ctrl+] withdraws the report so the parent shell's actual directory
can govern later splits. Shell panes do not report a fixed startup directory;
a later `cd` must take effect.

```bash
pwd -P                               # verify inside the new native split
rex block call -b <block> pwd         # source terminal's directory report
rex block call -b <block> process     # source foreground process
```

Directory reporting has isolated native evidence. Rex's actual app Split,
Herdr/Desktop splits and the configured remote path still need demonstration.
A displayed Task path is not evidence of the new shell's working directory.

## View first; take input explicitly

Conversation views observe live output or saved history. A live observer offers
Ctrl+R after the existing execution owner acknowledges its attachment; that
explicit action requests input on the same connection. Ctrl+] leaves the viewer.
Typing before taking input shows a read-only hint. Those keystrokes are discarded,
never queued for the conversation.
Use `lf resume <session-id>` to continue a conversation explicitly.

Creation of a native pane does not establish readiness. Report whether each
conversation connected, shows history, ended, failed, or remains uncertain.
A failed attachment must preserve the existing provider, drafts and pending
requests; it must not restart or replay them to make a new view appear ready.

LF Session execution and input ownership remain LF state. Rex workspace identity
and arrangement remain Rex state. Readiness belongs to one `lf open`: its records
are removed once that command reports. To view a conversation again, the receipt
offers another fresh launch (`lf open … --session <id>`); it never touches the
earlier tab.

## Failure and recovery

If native creation succeeds but its reply is lost, report that a workspace may
have appeared. Do not automatically retry the creation. Inspect and return to
that workspace in Rex. A subsequent explicit `lf open` requests another fresh
workspace, so it may create an additional view of the same Work.

An unreachable server is an error, not permission to launch on another server or
Machine. Partial failures preserve successful destinations and unrelated panes;
they do not trigger destructive rollback.

After a host restart, Rex restores what it supports. It may restore layout with
new shells; familiar pane IDs do not prove that programs survived. LF does not
replay commands into retained native panes. A fresh launch reads the Task's
current LF layout or config again.

## Retained Task terminals protect the checkout

A retained Rex terminal for a Task Session must prevent Task cleanup regardless
of focus. Only confirmed removal of the last relevant terminal can release its
hold, subject to other holds and normal cleanup conditions. Unavailable host
evidence is unknown, not absence. LF automatically closes only Desktop-owned
shells; Rex shells remain Rex's responsibility.

This integration remains unfinished, and nothing enforces it today. Live
conversation viewers in Rex are attached viewers, so the shared Session lifetime
owner already sees them. The cleanup consumer is LOO-717's `lf task presence`
lease, which is not merged. Rex's own inventory lists blocks by identity, so a
retained tab whose programs stopped can still be found by block id at cleanup
time without an LF registry; that query is not written. Requested work, drafts and
pending requests remain protected. Closing an unfinished Task view does not delete
its checkout or branches.

## Delivery boundaries

Rex, Herdr and cmux render fresh layouts, selected through shared host detection.
Desktop retains its current saved-layout behavior. Isolated native tests cover Rex
(fresh launches, Task directories, in-Rex caller, arrivals, closed arrivals) and
Herdr (fresh launches); cmux has fixture coverage only. tmux and plain-terminal
routes still open the conversation alone.

Before acceptance, demonstrate config templates, two fresh launches preserving
independent native edits, actual native splits after automatic and manual entry,
agent-tool host routing after viewer movement, passive arrivals, remote behavior,
live conversation input transfer and retained-terminal cleanup protection. Prior
registry-based reuse tests do not prove the revised fresh-launch contract.
