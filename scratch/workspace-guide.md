# Open your Loopflow workspace

> Documentation draft for LOO-498, requested by Jack Heart on November 3, 2026.
> This describes the agreed experience. The implementation is still in progress;
> the final section names the gaps that must be resolved before publishing it as
> a released feature guide.

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
submit an agent turn, or create another provider conversation.

## A Task is a tab with its tools

In Rex, each Task has a tab containing splits. A Wave is a Rex session containing
its conversation and Task tabs. Switch tabs to move between Tasks; rearrange
splits using Rex's normal controls.

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

```bash
lf task view LOO-498
```

In the terminal Task view, use `n` and `p` to select a step and `l` to read its log.
Viewing the Task does not start its work.

## Open again without starting over

```bash
lf open --task LOO-498
```

If that Task already has an occurrence in the destination, return to it. Keep
its splits, existing programs and unfinished input. Reopening does not reapply
the default layout.

For a new occurrence, choose its layout in this order:

1. The Task's saved layout.
2. Its Wave's default Task layout.
3. Loopflow's builtin layout.

A Wave default selects tools and their arrangement. Each host renders those
tools in its own way. Changing the Wave default affects future layouts; existing
Task customizations and live host arrangements remain intact.

The current layout interface can read or save a Wave default:

```bash
lf desktop layout wave-default:<wave-id>
lf desktop layout wave-default:<wave-id> --save < layout.json
```

Despite the command namespace, these are shared saved layouts. The current JSON
interface uses the Wave's durable ID and a revisioned pane tree; the command's
read output provides the shape to edit. It works without a running Desktop.

## New Tasks arrive when they start

Keep a repository or Wave workspace open while Tasks run. When a Task first
starts, it appears as a passive Task tab. Your focus stays where it is. Adding
the tab does not start another agent or a set of background shells.

Open that Task explicitly to activate its saved tools. Later Graph runs reuse
the same tab. Creating a backlog Task alone does not add it to the working set.
A workspace opened for one Task does not acquire sibling Tasks.

When you reopen a repository or Wave workspace, reconcile Tasks that started
while it was closed. Launching a Task does not unexpectedly open a host app.

The current Rex implementation follows Tasks through its Workspace tab. Closing
that tab stops following; opening the workspace again restores following.
Closing a Task tab leaves that occurrence closed during following.

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
The inspected public API has no scoped Task/window default. This needs a host
capability or an agreed interaction change before we can promise this behavior.

## Use your editor and diff tools

Terminal file editing uses `VISUAL`, then `EDITOR`, with `vi` as fallback. A
directory browser can be selected through `LF_FILE_BROWSER`. A configured GUI
editor may open its own window.

Changes uses Git's configured pager. The explicit difftool action uses Git's
diff-tool settings. Working-tree changes, branch comparisons and PR comparisons
keep their selected targets; viewing a committed comparison does not mix in
uncommitted edits.

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
server is different: the host may restore the layout with new shells. LF must
not treat familiar pane IDs as proof that the original programs survived or
replay commands over new work you have already started.

Host retention and Task cleanup are coordinated with LOO-717. Desktop and Rex
use the same LF retention contract; each host decides when to retain or release
its views. Requested final work finishes before cleanup. Closing an unfinished
Task view does not delete its checkout or branches.

For eligible completed-and-away Tasks, the agreed policy ends idle agents after
two minutes and retires shells/checkout at fifteen minutes total. Eligible cleanup
can discard uncommitted files. Task and conversation history remain. This lifecycle
integration is not yet delivered by the workspace adapter, and reliable Rex
release observations remain unresolved. Automatic recreation of a deleted
checkout is outside this feature.

## Host coverage and remaining delivery

| Host | Workspace coverage |
| --- | --- |
| Desktop | Existing Task workspaces; shared tools and Wave defaults |
| Rex | Primary adapter: Task tabs, Wave sessions, layouts, copies and automatic arrivals |
| Herdr | Bounded adapter currently implementing local selected-Task create/reuse/focus; Wave following and remote parity are not claimed |
| cmux, tmux, plain terminal | Existing conversation opening; the complete Task workspace adapter is outside this build |

The current branch has isolated native evidence for Rex opening/reuse, passive
arrival/activation, follower recovery, and Herdr Task creation/reuse. It still owes:

- Native new-split worktree defaults, described above.
- Safe restoration after a server replaces tool programs with login shells.
- Bare-open discovery on arbitrary non-default Rex servers, plus the actual
  agent-tool path after a viewer move. A direct native-shell test is not that proof.
- Following newly relevant remote Tasks and configured remote/app acceptance.
- Shared lifecycle integration, native cleanup and reliable release observations.
- Complete move/handoff and return/reopen demonstrations.

These are delivery gaps, not accepted reductions to the agreed experience.
