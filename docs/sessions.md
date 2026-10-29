# Sessions

You've run `lf resume --task LOO-12` and landed back in a Task's conversation.
Every conversation lf starts, interactive or headless, is a Session: it keeps
its name, its Task and its provider's native history, whichever terminal or
agent it ran in. This page covers finding them, steering them, and what
comes back after a crash.

## Come back to a conversation

```bash
lf resume                          # this checkout's ongoing conversation: its Task's, the Wave's, or the repository's
lf resume LOO-12                   # a Task's conversation, from anywhere
lf resume <session>                # an exact conversation
lf session list --available        # conversations available for your next turn, across this repository
```

`lf resume` always gets you back, with no flags to choose:

| The conversation is… | `lf resume` |
|---|---|
| running with no terminal showing it (`detached`) | reattaches; nothing restarts |
| shown in another lf terminal | takes it over; that terminal says where it went |
| running in a provider client lf started elsewhere | stops that client and continues here |
| ended or crashed | continues the same provider thread from its history |
| missing for this scope | starts one and says so |

Each scope (repository, Wave, Task) has one ongoing conversation, with that
scope's operating instructions.

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

## After a crash or reboot

```bash
lf resume --all
```

Every conversation a crash or restart cut off comes back, on the provider and
login it was using, with its native history. A conversation you exited or
detached from is left alone. A turn that was mid-flight is reported, not
replayed. Terminals attach when they open them.

| After a crash… | |
|---|---|
| interactive conversations | `lf resume --all` brings them back |
| Task worktrees, branches, PRs | untouched |
| comments and steers | saved; delivered at the agent's next boundary |
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
lf steer <session> "use the existing retry helper"   # saved; delivered at its next boundary
lf steer "freeze merges until 3pm"                   # every Session in this repository
lf --wave billing steer "invoices before refunds"    # every Session on the Wave
lf session steers <session>                          # was it delivered?
```

A steer starts no turn. Each Session receives it at its next supported provider
boundary; idle ones stay idle until they next run. For a Task, `lf comment`
does the same for every Session on it ([Work](work.md#steer-it-while-it-runs)).
Delivery proves the agent saw the steer, not that it followed it;
`lf session steers <session> --retry <steer>` retries an uncertain delivery.

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
lf session bind <session> --task LOO-12      # file a loose conversation under its Task
```

New Sessions are named from the request or the skill; Task conversations take
the Task's title. A name you set is never replaced by a suggestion
(`rename --suggest` proposes one). Interactive terminal titles follow the name,
Task identifier first. In cmux, a Task's workspace is named
`<Task title> · <identifier>` and changes only when the Task title does; a
workspace you renamed in cmux keeps your name.

Launching in a Task's checkout binds the Session to that Task automatically.
`bind` is for the rest: it assigns a Task to a Session that has none,
permanently, including a done or landed Task, without reopening it.
`--dry-run` shows the exact target first.

## Which login it runs on

```bash
lf --account claude=work@ : "draft the migration"     # prefer this login, fall back to the route
lf --only-account claude=personal@ -b task run LOO-12 # this login only, for every step and child
```

A new launch tries the repository's account route, then your default route,
then any eligible login with room. A resumed Session stays on the login it
started with. A Graph carries its selection to every step; run a fresh Graph to
choose again. Connecting logins and setting routes: [Ops](ops.md).

## Reference

**Session selectors.** `resume`, `rename` and `bind` take a Session ID or a
provider conversation ID; `resume` also takes a Task identifier, which opens
that Task's conversation. `lf task hold` and `lf task release` are about the
Task's runs, not its conversation.

**One primary per scope.** A Task keeps its selected primary conversation;
opening or forking another Session doesn't change it.

**Preparing a terminal.** `lf resume <session> --prepare --json` returns the
command to open it without starting the provider (with `--fork`, the fork's);
for a remote machine, an SSH command. Running that command takes over as
`lf resume` does.

**Native history.** `lf session history <session>` reads the provider's start,
usage and completion receipts.

Every flag: [`lf session`](lf-reference.md#lf-session).

Next: [Ops](ops.md), landing what those conversations built.
