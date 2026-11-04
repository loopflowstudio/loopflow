# Open your Loopflow workspace

> In development. This guide describes the intended workspace experience.
> The final section lists the capabilities still being implemented or verified.
> The ownership boundary below is accepted; retention integration remains unfinished.

```bash
lf open                              # open the Work selected by your current context
lf open --wave product               # open Product's started, unfinished Tasks
lf open --task LOO-498                # open one Task
lf open --host rex                   # choose Rex explicitly
```

Run `lf open` from the repository checkout to open its working set, grouped by
Wave. From a Task checkout, it opens that Task. Use `--wave` or `--task` to select
Work explicitly. Tasks without a Wave have their own group.

Inside Rex, opening stays in the appropriate Rex server and session. The same
command works when an agent runs it through its tool shell. An explicit `--host`
overrides automatic host selection. A remote Task stays on its recorded Machine;
an unavailable Machine is reported rather than replaced with a local shell.

Opening a workspace presents existing Work. It does not launch a Task's Graph,
submit an agent turn, or create another provider conversation. Use `--server`
to select a local Rex Unix server explicitly and `--no-focus` to preserve its
selected window. Server creation and an accepted display request do not prove
that the app is visible or the conversation ready for input.

## A Task is a tab with its tools

In Rex, each Task has a tab containing splits. A Wave is a Rex session containing
its existing conversation, when present, and Task tabs. Switch tabs to move between
Tasks; rearrange splits using Rex's normal controls.

Loopflow stores its own layouts and templates. Opening in Rex asks it to
recreate the selected layout as faithfully as its tools allow. After creation,
Rex owns that workspace: its splits, resizing, rearrangement and saved state.
Rex edits never write back into Loopflow's layout records. Loopflow does not
save or restore those edits on Rex's behalf.

The tools have the same purpose across hosts:

| Tool | What you work with |
| --- | --- |
| Conversation | The Task's existing LF conversation |
| Shell | A shell in the Task's worktree, on its recorded Machine |
| Files | The Task's files, through a file browser or editor |
| Changes | Working-tree changes or the selected branch/PR comparison |
| Task & workflow | The brief, current workflow step, runs, results and logs |

Conversation and Shell are terminals in both Desktop and Rex. Desktop supplies
native views for Files, Changes and Task/workflow; terminal hosts use the chosen
programs and LF's terminal view.

Opening a Conversation in Rex observes its live output or saved history.
Use `lf resume <session-id>` when you want to take conversation input.

```bash
lf task view LOO-498
```

In the terminal Task view, use `n` and `p` to select a step and `l` to read its log.
Viewing the Task does not start its work. Without a conversation, the builtin
layout shows Task & workflow beside a shell.

## Open again without starting over

```bash
lf open --task LOO-498
```

If that Task already has an occurrence in the destination, return to it. Keep
its splits, existing programs and unfinished input. Reopening does not reapply
the default layout. Rex owns subsequent arrangement and restoration. If the tab
is confirmed gone, an explicit open creates a fresh view from LF's template.
An unavailable host or an uncertain creation reply cannot establish absence;
inspect the native result before retrying.

For a new occurrence, choose its layout in this order:

1. The Task's saved Loopflow layout.
2. Its Wave's default Task layout.
3. Loopflow's builtin layout.

A Wave default selects tools and their arrangement. Each host renders those
tools in its own way, with explicit differences where a tool cannot be rendered.
This is an initial template, not a layout synchronization contract. Changing the
Wave default affects future layouts; existing Task customizations and live host
arrangements remain intact. Only selected, visible tools materialize.
File drafts and background tabs remain in the saved
source layout; a selected web page is reported as unsupported by the terminal
renderer. Templates never capture a Task's Sessions, execution locations or drafts.

The current layout interface can read or save a Wave default:

```bash
lf desktop layout wave-default:<wave-id>
lf desktop layout wave-default:<wave-id> --save < layout.json
```

Despite the command namespace, these are shared saved layouts. The current JSON
interface uses the Wave's durable ID and a revisioned pane tree; the command's
read output provides the shape to edit. `hidden` lists hidden pane IDs. It works
without a running Desktop. Changes tabs carry `working_tree` with `base`,
`branch` with resolved `base`/`head` SHAs, or `pull_request` with those SHAs and
its `number`.

## New Tasks arrive when they start

Keep a repository or Wave workspace open while Tasks run. When a Task first
starts, it appears as a passive Task tab. Your focus stays where it is. Adding
the tab does not start another agent or a set of background shells.

Open that Task explicitly to add its saved tools once beside the passive pane.
The passive pane runs in the Task checkout. Later Graph runs reuse the same tab.
Following only adds Tasks; it does not rearrange tabs, repair their programs or
remove them when Tasks finish. Creating a backlog Task alone does not add it to
the working set.
A workspace opened for one Task does not acquire sibling Tasks.

When you reopen a repository or Wave workspace, reconcile Tasks that started
while it was closed. Launching a Task does not unexpectedly open a host app.

The current Rex implementation follows Tasks through its Workspace tab. Closing
that tab stops following; opening the workspace again restores following.
Closing a Task tab leaves that occurrence closed during following. An exited
follower is replaced in a new Workspace tab; its old native tab stays intact.

The follower asks a remote Task's recorded Machine whether it has started, even
when the local plan has no checkout for it. Remote arrivals use the same passive
Task view and reuse rules. A failed connection leaves the source workspace intact;
reconciliation retries without creating local replacement shells. Configured
two-Machine interaction still needs demonstration.

## New splits belong to the selected Task

Use the host's normal split shortcut or menu from a Task tab. The new shell
should start at that Task's worktree root on the same execution Machine—even
when you split beside a conversation, file view or Task view.

If a Wave contains Tasks A and B, splitting in A must enter A's checkout;
splitting in B must enter B's. There is no single Wave-wide working directory.
Explicit directory choices remain yours.

**This requirement is not implemented yet.** In the installed Rex version, the
isolated native split test starts both new shells in home when no cwd is supplied.
Correct directories on LF's initial panes do not solve subsequent native splits.
The inspected public API has no scoped Task/window default. The app's normal
Split action may supply its own directory; that path still needs verification
before concluding that a host change or different interaction is required.

## Use your editor and diff tools

```bash
lf task tool LOO-498 files --checkout /recorded/checkout
lf task tool LOO-498 changes --checkout /recorded/checkout --base parent --difftool
```

Terminal file editing uses `VISUAL`, then `EDITOR`, with `vi` as fallback. A
directory browser can be selected through `LF_FILE_BROWSER`. A configured GUI
editor may open its own window.

Changes uses Git's configured pager, including untracked files in the patch.
The explicit difftool action uses Git's diff-tool settings for tracked files.
Working-tree changes, branch comparisons and PR comparisons
keep their selected targets; viewing a committed comparison does not mix in
uncommitted edits. `--head <sha>` selects the committed comparison explicitly.

Resolve these settings on the Task's execution Machine, in its repository.
You do not need separate editor and diff preferences just because you use Rex.

## Open another view of the same Work

```bash
lf open --host rex --task LOO-498 --copy review
```

The named copy refers to the same Task and conversation. It starts as an observing
view. Repeating the command reuses that copy. Use a different name for another
copy; add `--new-shells` when creating it if you want fresh ordinary shells.

```bash
lf resume <session-id>
```

Taking conversation input is explicit. Copying a layout does not copy running
jobs, shell variables, editor memory or unfinished shell commands. Rearranging
native tabs preserves their programs. Opening in another host leaves source
shells and unsaved work with their original host.

## Reopen, restore and close

Returning to a retained workspace reuses its native views. Restarting the host
server is Rex's restoration responsibility: the host may restore the layout with
new shells. Familiar pane IDs do not prove that the original programs survived;
LF does not replay commands over new work you have already started.

An explicit fresh launch can recreate the Loopflow template as faithfully as
the host supports. It does not recover edits made only in Rex or reconstruct
running programs from a saved layout.

LF automatically cleans up shells owned by Loopflow Desktop. It does not close
Rex shells or tabs.

A Rex terminal retained for a Task Session keeps that Task from being cleaned up,
even when another tab is selected or Rex is in the background. Once the last such
terminal is confirmed gone, normal Task cleanup conditions apply. An unavailable
Rex server does not count as confirmation that its terminals are gone.

Requested work finishes before cleanup. Closing an unfinished Task view does not
delete its checkout or branches. Desktop's existing lifetime policy remains;
Task and conversation history survive cleanup. This retention integration is not
yet delivered. Automatic reconstruction of a deleted checkout is outside scope.

## Host coverage and remaining delivery

| Host | Workspace coverage |
| --- | --- |
| Desktop | Existing Task workspaces; shared tools and Wave defaults |
| Rex | Primary adapter: Task tabs, Wave sessions, layouts, copies and automatic arrivals |
| Herdr | Local selected-Task create/reuse/focus through the current `HERDR_SOCKET_PATH`; Wave following, remote forwarding and live process migration are outside this adapter |
| cmux, tmux, plain terminal | Existing conversation opening; the complete Task workspace adapter is outside this build |

The current branch has isolated native evidence for Rex opening/reuse, passive
arrival/activation, follower recovery, and Herdr Task creation/reuse. It still owes:

- Native new-split worktree defaults, described above.
- Clear template relaunch and native reuse behavior after a server restart,
  without claiming to restore Rex edits or replaying commands into existing work.
- Bare-open discovery on arbitrary non-default Rex servers, plus the actual
  agent-tool path after a viewer move. A direct native-shell test is not that proof.
- Configured remote arrival, recovery and app acceptance across two Machines.
- Retained Rex Task Session terminals preventing cleanup, with reliable release
  observation and no LF-initiated closure of Rex shells.
- Complete move/handoff and return/reopen demonstrations.

These are delivery gaps, not accepted reductions to the agreed experience.
