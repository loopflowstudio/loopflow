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
Once a live view connects, press **Ctrl+R** to take input in that pane.
**Ctrl+]** leaves the viewer. An observing copy stays passive until that explicit
action. For a conversation showing only saved history, use `lf resume <session-id>`.

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

Opening reports successful Task destinations alongside native creation failures.
If one Wave destination fails, other Task destinations can still open; retrying
preserves the successful native work.

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

Tools created by LF retain the destination server when they launch, so a shell
opened there can use bare `lf open` to return to the same Work. This does not yet
cover discovering every pre-existing terminal on an arbitrary custom server.
Native terminals can discover an explicit listener from their Rex server's process
ancestry. If that evidence is unavailable, opening reports it; it does not query
a different configured server. Use `--server` to select the destination explicitly.

The follower asks a remote Task's recorded Machine whether it has started, even
when the local plan has no checkout for it. Remote arrivals use the same passive
Task view and reuse rules. A failed connection leaves the source workspace intact;
reconciliation retries without creating local replacement shells. A connected
fixture covers reconnecting and recovering a lost successful reply without local
Work changes or duplicate panes. Configured two-Machine interaction still needs
demonstration.

## New splits belong to the selected Task

Use the host's normal split shortcut or menu from a Task tab. The new shell
should start at that Task's worktree root on the same execution Machine—even
when you split beside a conversation, file view or Task view.

If a Wave contains Tasks A and B, splitting in A must enter A's checkout;
splitting in B must enter B's. There is no single Wave-wide working directory.
Explicit directory choices remain yours.

The same holds when LF opens the workspace and when you start or resume a Task
yourself in an existing terminal, from any directory. While LF is attached to
that terminal, it runs in the Task's directory and reports that directory, so
Rex has one answer. A conversation bound to another Task checkout after it
started reports the selected checkout the next time it is opened.

Leave with Ctrl+] and your shell has the terminal back: LF withdraws its report,
so a split follows your shell's real directory, including any `cd` you make
afterward. A Shell pane never reports a fixed directory, for the same reason.

Rex's Split uses the source terminal's directory when it has one, and otherwise
the server's default directory. Check what a pane offers:

```bash
rex block call -b <block> pwd       # what the terminal reported
rex block call -b <block> process   # where its foreground process runs
```

A displayed Task path proves neither. **Confirming this with Rex's Split control
is still owed;** isolated checks cover both directory sources, not the app's choice
between them. Herdr and Desktop splits have not been checked.

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

The live destination first connects as an observer. Ctrl+R then requests input
through the existing Session execution owner, on that same connection. Input
typed before either acknowledgement is discarded. A failed preparation leaves
the source in control; an unanswered transfer is reported as uncertain and is
never retried automatically. A native create receipt alone does not take input.
Rex keeps Task and Wave Conversation terminals visible when their observers exit,
including saved history and preparation errors. Reopening reuses them without
replaying their commands; use an explicit resume to continue an ended conversation.

Each opening reports a receipt for the native destination and, separately, a
state for every Conversation observer in it:

| State | Meaning |
| --- | --- |
| `connected` | The Session's execution owner acknowledged the observer, and it still runs |
| `history` | No live execution exists; the pane shows saved history |
| `failed` | The observer could not attach, or it no longer runs |
| `uncertain` | The owner never answered, or the observer has not reported; inspect the destination |

A native create or reuse never counts as readiness. `lf open` waits briefly for
observers it just launched, so a slow attachment can read `uncertain` and settle
later. Opening again reports the current state.

Reopening a retained tab does not restart an exited observer. Its pane stays as
it was, including any error. A `failed` or `uncertain` observer that has stopped
gets a retry command in the report:

```bash
lf open --host rex --task LOO-498 --copy review --retry-observers
```

This adds a new observer beside the retained pane in the same destination. It
replays nothing into that pane, takes no input and never takes focus. If Rex's
reply to the retry is lost, the observer reads `uncertain` and retrying adds no
second pane until it reports. Ctrl+R in the new pane is still how input moves.

If reconnection fails while a turn, draft or pending request keeps the provider
active, `lf resume` reports the failure and preserves that work. Saved history
cannot reconstruct unfinished input or answer a pending request.

## Reopen, restore and close

Returning to a retained workspace reuses its native views. Restarting the host
server is Rex's restoration responsibility: the host may restore the layout with
new shells. Familiar pane IDs do not prove that the original programs survived;
LF does not replay commands over new work you have already started. Opening
after a restart reuses every destination Rex still has and reports its stopped
Conversation observers as `failed`, with the retry command above.

Once a destination is confirmed gone, opening creates it again from the Loopflow
template, as faithfully as the host supports. That does not recover edits made
only in Rex or reconstruct running programs from a saved layout. An unreachable
Rex server confirms nothing: opening reports the failure and creates nothing. A
creation whose reply was lost stays unknown across a restart until you inspect
Rex; no duplicate is attempted.

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

- Confirming native Split from each pane in the Rex app, and Herdr/Desktop split
  behavior. LF's directory reporting is implemented, as described above.
- Bare-open discovery covers explicit listeners on the native Rex server ancestry.
  Servers with unreadable ancestry and the actual agent-tool path after a viewer
  move still need proof; a direct native-shell test is not that proof.
- Configured remote arrival, recovery and app acceptance across two Machines.
- Retained Rex Task Session terminals preventing cleanup, with reliable release
  observation and no LF-initiated closure of Rex shells.
- Configured move/handoff and return/reopen demonstrations with a real provider.

These are delivery gaps, not accepted reductions to the agreed experience.
