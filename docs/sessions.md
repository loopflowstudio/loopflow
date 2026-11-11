# Sessions

You've run `lf resume --task LOO-12` and landed back in a Task's conversation.
Every conversation lf starts, interactive or headless, is a Session: it keeps
its name, its Task and its provider's native history, whichever terminal or
agent it ran in. This page covers finding them, steering them, what keeps
running through an update, and what comes back after a crash.

## Open without moving another view

```bash
lf open --task LOO-12              # the primary conversation in this terminal
lf open --session <session> --tab  # an exact Session in a sibling tab
lf open --task LOO-12 --focus       # reveal its existing view
```

`lf open` reports an already-open conversation and offers `--focus`; it does not
take input from another view. Agent calls default to sibling tabs. To deliberately
move an existing conversation's input here, use `lf resume`. Both continue an
ended Session from history without replaying its prompt. Presentation modes and
host support: [Open conversations and workspaces](workspaces.md).

## Come back to a conversation

```bash
lf resume                          # this checkout's ongoing conversation: its Task's, the Wave's, or the repository's
lf resume LOO-12                   # a Task's conversation, from anywhere
lf resume <session>                # an exact conversation
lf session list --available        # conversations available for your next turn, across this repository
```

`lf resume` reconnects to the conversation's running provider or continues its
saved thread after safe replacement.

A live turn, draft or pending request keeps its provider. If attachment fails,
resume reports the failure and leaves that work intact. Returning does not
resend input.

| The conversation is… | `lf resume` |
|---|---|
| running with no terminal showing it (`detached`) | reattaches; nothing restarts |
| shown in another lf terminal | takes it over; that terminal says where it went |
| running in a provider client lf started elsewhere | reconnects to retained work; continues from history when the client can safely be replaced |
| ended or crashed | continues the same provider thread from its history |
| missing for this scope | starts one and says so |

Each scope (repository, Wave, Task) has one ongoing conversation, with that
scope's operating instructions.

A Task, Wave or repository conversation runs until its Task is done or
abandoned or its agent exits; closing every view does not end it. Any other unviewed, settled conversation ends its
processes after two minutes of quiet; `session.end_detached_after` changes the
grace or disables it with `off`.
Returning paints its saved screen while the provider reopens its history, without
replaying input. Active turns, subscriptions, running Graphs, drafts and pending requests
keep their owner. Drafts remain editable and requests remain answerable there.
Edited, pasted or unacknowledged native input can retain the owner until the
provider exits; saved history and screens cannot restore an editor or request.

```bash
lf resume <session> --fork         # leave the other client running; continue in a new conversation from it
lf session new                     # restart this scope's conversation on a fresh thread, seeded with what the last one knew
```

`--fork` makes a new Session in the same Work, starting from the conversation's
log. `lf session new` keeps the same Session and name: a new provider thread
starts from that log and current instructions, and the old thread stays in
`lf session history`.

`lf session list --all` covers every repository on this machine; `--history`
adds finished conversations, and `--search` narrows the list.

## Keep working through an update

```bash
lf resume --task LOO-12            # terminal one: a turn is streaming
lf install                         # terminal two, mid-turn
```

The turn keeps streaming. The terminal showing it waits, drawing nothing and
discarding what is typed, while the server is replaced; a Desktop pane on the
same conversation says **Reconnecting…**. Each then continues where it left
off, showing what the agent printed meanwhile. Nothing is resent to the agent.
An agent that exits meanwhile ends its view with its own exit code, as if no
server had been away: `The agent exited with 3.`

A Task or Wave conversation's agent runs in a holder: one `lf holder run`
process beside the server that keeps the terminals and agents of every such
conversation. The server restarts on every install; a holder is never
restarted for one. With ten Task conversations open, `ps` shows one
`lf server run`, one `lf holder run` and ten agents; `lf ps` lists the
holder once.

| Opened with | Runs in |
|---|---|
| `lf task run`, `lf ship`, `lf resume --task`, a Wave session command | the holder |
| a Desktop pane | the holder |
| `lf resume <session>` of a conversation one of those opened | the holder |
| `lf <skill>`, `lf graph …`, `lf -i`, anything with `-b` or a pipe, even in a Task's checkout | its own process, as before |
| a Desktop Task shell | its own process, as before |

A holder keeps the build it started with. After an install, conversations
already open stay where they are and new ones start in a holder on the new
build, so two or more holders can be alive at once. Each exits a minute after
its last conversation ends.

| When | Agents | Open views | A view opened afterward |
|---|---|---|---|
| `lf install` | keep running | pause, then continue with what was printed meanwhile | the current screen; older scrollback may be missing |
| the server crashes or is killed | keep running | pause until the OS restarts the server, then continue | the current screen; older scrollback may be missing |
| a holder is killed | end, with the conversations it held | say the process is gone and offer resume | `lf resume` continues from provider history |
| logout or reboot | end | close | [resume from saved history](#after-a-crash-or-reboot) |

While the server is away a holder keeps the newest 2 MiB of each
conversation's output, and the agent never waits on it. A new server
rebuilds each screen from that output, then asks the agent to redraw by
changing the terminal's width by one column and back: the visible screen is
exact and scrollback reaches as far back as the kept output. A view in a
terminal waits without drawing anything, discards what is typed meanwhile,
and continues when the server is back; Ctrl+] closes it. A view away longer
than the kept output covers repaints the current screen instead.

A conversation that ends while the server is away is recorded with its real
exit when the server returns; until then `lf session list` shows what it last
knew.

These conversations need the server. Opening one while it is stopped starts
the server first; if it still does not answer, lf says
`Loopflow server is not running`, with what starting it said, and starts
nothing.
No private process is started in its place. A Graph already running keeps
going without it.

A conversation opened before this arrived keeps the `lf session serve`
process it started with until it ends.

## When Desktop loses its connection

Desktop reconnects to the same live owner automatically and shows connection
status outside the terminal. Typing pauses while the screen remains available.
A newer viewer keeps control until an explicit **Move here** request. **Retry**
repairs connectivity; it does not resume a new provider or replay input.
[Connect clients](server.md) covers server updates and connection diagnosis.

A view of a Task or Wave conversation continues from its place in the kept
output, without redrawing; one that fell behind it gets a fresh screen. For
conversations in their own process, byte-stream viewers catch up from retained
output when available and terminal viewers refresh the screen from the same
owner. Another viewer's
input and terminal size stay in effect. If the old owner has ended, recovery
stops at that owner; opening the Session again can reach its current owner.
Reconnecting a display never starts a provider.

The pane's connection row explains which conversation came back:

| What appears | What continued |
|---|---|
| **Same process** | The existing provider process is still serving the conversation. |
| **Resumed in a new process** | A replacement process reopened the same provider conversation. |
| **Forked from …** | A new provider conversation carries history from the source. “Original was running at fork” describes that moment, not its current status. |
| **New conversation; earlier one left behind** | The new provider conversation does not carry the earlier history, as can happen on account failover. |

`lf session history <session>` keeps the observed provider changes. Unknown
history stays unknown; a connection alone does not establish continuity. Reading
that history or a terminal screen grants no input control.

Starting a new thread also uses
[submitted-creation recovery](server.md#recover-a-submitted-creation). Returning
after a lost reply does not stop a newer provider or append the same input again.
An interrupted stop shows **Conversation change unconfirmed** and keeps Inspect
available. A confirmed result clears its saved submission when it appears.

## After a crash or reboot

```bash
lf resume --task <ID>
```

Open the conversation you want, on the provider and login it was using, with
its native history. Resuming does not replay a turn cut off by the crash.
Other conversations stay stopped until opened.

| After a crash… | |
|---|---|
| interactive conversations | `lf resume --task <ID>` or `lf resume SESSION` |
| only the server crashed | nothing to do: agents kept running and views reconnect |
| Task worktrees, branches, PRs | untouched |
| steers and reports | saved; delivered at the agent's next boundary |
| a headless Graph that died | kept as history; nothing resumes it. `lf task status`, then `lf task run` what remains |

`lf resume` takes Sessions. A Run (`-b`, a Graph step), running or finished, is
refused with a pointer to `lf resume --task <task>`; the Task's Session reads its log.

## Bring in one you started elsewhere

```bash
lf resume 0199a213-81c0-7800…      # a conversation from plain `claude` or `codex`
```

A provider's own conversation ID names a Session wherever a Loopflow ID does.
One the provider started on its own is found in its home and becomes a Session
the first time you resume it: from then on it lists, renames, binds and resumes
like any other.

A raw `claude` or `codex` still open on it isn't one lf can stop; close it, or
`--fork` to leave it running.

## Switch agents mid-conversation

```bash
lf -a codex resume <session>       # the same Session, continued on Codex
lf session new -a claude           # this scope's conversation, continued on Claude
```

The Session keeps its name, Task and history. The new agent starts from a plain
log of the conversation so far (what was asked, what was answered, one line per
tool call), not a summary. On the harness it already uses, `-a` changes nothing.

## Steer without interrupting

```bash
lf session steer <session> "use the existing retry helper"   # saved; delivered at its next boundary
lf session steer "freeze merges until 3pm"                   # every Session in this repository
lf --wave billing session steer "invoices before refunds"    # every Session on the Wave
lf session steers <session>                                  # was it delivered?
```

A steer starts no turn. Each Session receives it at its next supported provider
boundary; idle ones stay idle until they next run. For a Task, `lf task steer`
does the same for every Session on it ([Work](work.md#steer-it-while-it-runs)).
Delivery proves the agent saw the steer, not that it followed it;
`lf session steers <session> --retry <steer>` retries an uncertain delivery.

## Keep several conversations open

Leave Task conversations open while their Graphs run. LF watches for new
comments and progress; once work settles, idle update checks read revision
counters instead of rebuilding unchanged Task history on every clock tick.
This is automatic and needs no setting.
Transcript, usage and ordinary tool activity in other conversations do not
trigger those history rebuilds. A comment or workflow change on another Task
can still do so because observers share the workflow revision counter.

Locally saved comments can reach a conversation even when planning connection
settings are unavailable. Sync badges remain in the Task thread. A busy provider
leaves an update pending for retry. A failed store read is retried even if nothing else changes;
lf records the gap in `logs/lf.log` while the provider conversation continues.
Live Graphs still get periodic checks so an unrecorded process exit can be noticed.

## One more headless turn

```bash
lf -b resume <session> --message "now add the migration test"
```

Runs the next turn in the Session's saved workspace, from any directory, with
its native history and without re-running the original skill. Output prints and
the command returns when the turn ends.

## Name it and file it

```bash
lf session rename <session> "Release notes"
lf session bind <session> --task LOO-12      # select the Task’s primary conversation
```

New Sessions are named from the request or the skill; Task conversations take
the Task's title. A name you set is never replaced by a suggestion
(`rename --suggest` proposes one). Interactive terminal titles follow the name,
Task identifier first. In cmux, a Task's workspace is named
`<Task title> · <identifier>` and changes only when the Task title does; a
workspace you renamed in cmux keeps your name.

Launching in a Task's checkout binds the Session to that Task automatically.
`bind` permanently associates an unfinished interactive Session and selects it
as the Task's primary conversation. Repeating it selects that same Session again;
other conversations stay associated. A Task with a worktree keeps it, and binding
immediately sets the Session's working directory there. Already-running clients
may keep their original directory; subsequent connections use the selected one.
Files stay in their old checkout.
An open Task without a worktree adopts the Session's unowned linked checkout;
main and Wave checkouts stay shared. Done Tasks can still select an unfinished
conversation. If the Task has no checkout, the conversation stays in place and
no checkout is adopted. Binding starts no Graph, and leaves done or landed Tasks
closed. Closed Sessions and headless Runs cannot become primary.
`--dry-run` shows the exact target first.

## Which login it runs on

```bash
lf --account claude=work@ : "draft the migration"     # prefer this login, fall back to the route
lf --only-account claude=personal@ -b task run LOO-12 # this login only, for every step and child
```

A new launch tries the repository's account route, then your default route,
then any eligible login with room. A resumed Session runs on the active login.
A Graph carries its selection to every step; run a fresh Graph to choose
again. Connecting logins and setting routes: [Ops](ops.md).

## Move what a limit blocks

```bash
lf session list --blocked                    # whose next message would fail
lf session move --blocked -a claude:opus     # continue them on Opus
lf account use claude=work@                  # switch the Machine's login and stay there; every open Claude follows
```

`--blocked` and `--on <account|harness[:model]>` select Sessions; `list`
previews what an `-a` move moves. An idle Session continues on the new agent at
its next open, seeded with its conversation; a busy one moves after its turn.
Logins belong to the Machine, not to a Session: every open Claude runs on the
active one. A running Codex keeps its login until you restart it on another:
`lf --account codex=<login> resume <session>`. `--blocked` and `--on` read the
login each Session uses. `use` holds launches there until the login fills up
or `lf account use auto`.

Interactive Sessions only; nothing is written to your preferences. More: [Subscriptions](subscriptions.md#when-an-account-runs-out).

## Reference

**Session selectors.** `resume`, `rename` and `bind` take a Session ID or a
provider conversation ID; `resume` also takes a Task identifier, which opens
that Task's conversation. `lf task claim` and `lf task unassign` are about who
owns the Task, not its conversation.

**One primary per scope.** A Task keeps its selected primary conversation;
opening or forking another Session doesn't change it.

**Preparing a terminal.** `lf resume <session> --prepare --json` returns the
command to open it without starting the provider (with `--fork`, the fork's);
for a remote machine, an SSH command. Running that command takes over as
`lf resume` does.

**Native history.** `lf session history <session>` reads the provider's start,
usage and completion receipts.

Every flag: [`lf session`](lf-reference.md#lf-session).

Next: [Pull requests](ops.md#pull-requests), landing what those conversations built.
