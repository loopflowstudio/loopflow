# Sessions

You've run `lf resume --task LOO-12` and landed back in a Task's conversation.
Every conversation lf starts, interactive or headless, is a Session: it keeps
its name, its Task and its provider's native history, whichever terminal or
agent it ran in. This page covers finding them, steering them, what keeps
running through an update, and what comes back after a crash.

## Resume or open another view

```bash
lf resume                          # this checkout's ongoing conversation (Task, Wave, or repo)
lf resume LOO-12                   # a Task's conversation, from anywhere
lf resume <session>                # an exact Session or provider conversation ID
lf resume LOO-12 --unfocused       # attach without assuming this terminal has focus
lf open --task LOO-12 --tab        # attach a second view in a sibling tab
lf open --task LOO-12 --focus      # reveal an existing view without opening a new one
```

A conversation can be open in multiple terminals (and Desktop) at once, and
every view can type: a draft started in one appears in the others. Size, colors,
and the agent's focus follow the view you are using: the live view you most
recently focused or typed in. Running `lf resume` in a terminal takes the size
right away. Desktop panes start in the background. Views with piped stdin and
terminals opened with `--unfocused` also attach without assuming focus. They take the size when the terminal reports
focus, you type, or the current owner leaves and they are next in line.
Ghostty reports focus on attach if its pane is already focused. A reconnecting
view uses the last focus it observed. See [Workspaces](workspaces.md) for host
support (`desktop`, `rex`, `cmux`, `herdr`, `tmux`, `terminal`).

| If the conversation is… | `lf resume` does… |
|---|---|
| Missing for this scope | Starts a new one |
| Running in a held terminal (shown or hidden) | Attaches another view here; existing views stay open |
| Ended or crashed | Continues the provider thread it saved (paints last screen while reopening), or starts again from its first request |
| Running in an unshared terminal (`lf <skill>` elsewhere) | Takes it over: ends the other client and continues here |

Need a parallel branch of the same conversation, or a fresh context window?

```bash
lf resume <session> --fork         # new Session in the same Work from this conversation's log
lf session new                     # fresh provider thread for this scope, seeded with what the last one knew
```

Wave and repository conversations run until you
quit their agent. Task conversations stay alive while the Task is open. Once a
Task completes, its unviewed conversation closes about 15 minutes after live
work settles (returning later continues from saved history without reopening
the Task). Abandoning a Task stops unviewed conversations immediately.

## Four kinds of launch

Loopflow names every launch by two choices: interactive vs. headless, and
managed by the local server vs. bare:

| Kind | Started by | Terminal & lifetime | Server involvement |
|---|---|---|---|
| **Work session** | `lf resume --task`, interactive Graph steps, Wave/repo sessions, Desktop panes | Runs in a **holder** process; outlives windows and `lf install` | Records turns, wakes idle conversations on steers/updates |
| **Worker** | Headless steps of `lf task run` / `lf task auto on`, Discord Wave replies | Headless; runs in its own process | Records turns; keeps running if server stops |
| **Chat** | `lf`, `lf <skill>`, `lf -i`, interactive `lf graph` | Runs directly in your terminal (`Ctrl+C`/`Ctrl+Z` go straight to the agent); ends with the terminal | None; reads transcript at exit |
| **Batch** | `lf -b <skill>` | Headless child of its caller; prints output and exits | None; reads transcript at exit |

A Session can change kind between launches: for example, binding a chat to a
Task reopens it as a work session next time.

Running `lf` from inside an existing Claude Code or Codex conversation always
starts an independent Session (never a nested child), inheriting your
`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, and API keys without leaking the parent
conversation's session markers.

## Survive updates without interrupting work

```bash
lf resume --task LOO-12            # terminal 1: agent is mid-turn
lf install                         # terminal 2: upgrade lf
```

The agent keeps streaming. Your terminal pauses briefly (Desktop shows
**Reconnecting…**) while `lf install` swaps the server, then continues right
where it left off with everything printed in the meantime. Nothing is resent.

This works because work sessions run inside a separate `lf holder run` process
beside the server. `lf install` restarts the server and never restarts a
holder: existing conversations stay on the build they started with, new ones
start in a holder on the new build, and each old holder exits a minute after
its last conversation ends. While the server is away, the holder buffers the
newest 2 MiB of output per conversation.

| Event | Running agents | Open views |
|---|---|---|
| `lf install` or server crash | Keep running | Pause, then catch up from buffered output |
| Agent process killed | That agent ends; a Task's work session restarts automatically | Report exit, then reconnect |
| Holder killed or reboot | End; Task work sessions and runs restart automatically | Close / reconnect |

To move a long-running conversation onto a newer `lf` build, quit its agent and
run `lf resume`.

## Auto-recovery after a kill, crash, or reboot

```bash
lf task status LOO-12
# Conversation: died 12:01 (exit 137) · starting again
# Conversation: running (started again 12:01, attempt 1)
```

- **Task work sessions** that die without you quitting them (killed, crashed,
  logout, reboot) are restarted automatically by the server from their saved
  thread on the same provider and login. Cut-off turns are never replayed;
  pending steers arrive at the first boundary.
- **Task runs** (`lf task run`, `lf task auto on`, and their workers) are carried
  again as `lf task run` from the step they reached. Interactive stops stay
  stops, and recovery never turns auto on.
- **Work sessions you quit** (`/exit`, `Ctrl+D`, `Ctrl+C` at the prompt) stay
  quit until you run `lf resume --task <ID>`.
- **Backoff.** After a reboot, the server brings Tasks back one at a time, most
  recently active first. Repeated deaths back off from 1 minute up to 30
  minutes and stop after 5 attempts without a finished turn or step, printing
  the command to restart manually (which resets the counter).
- **Missing transcripts.** A launch can end before the provider saves anything
  (`claude` missing from `PATH`, a failed login or trust prompt, a crash, Ctrl+C
  before you typed), or the transcript can be deleted. Opening it again, from
  the server, `lf resume` or a pane, starts a fresh thread with its original
  request; the old thread stays in `lf session history`. A saved conversation
  always resumes its thread; if the provider refuses, the Session stays as it was.

  ```bash
  lf resume 7f3a2c
  # This conversation has no saved claude transcript; starting it again from its first request.
  ```

`lf machine doctor` reports remaining PTYs (each held work session uses one).
In `--json`, each Session carries its `outlook` (`starting`, `coming_back`,
`waiting_for_you`, `quit`, `could_not_start`) and a ready-to-display
`status_line`.

Chats, batches, and Wave/repo conversations do not auto-restart; reopen them
with `lf resume`.

## See what every conversation is doing

```bash
lf session list                    # open interactive conversations in this repo
lf session list --available        # conversations ready for your next turn
lf session list --headless         # include running headless workers
lf session list --all              # across every repository on this Machine
lf session card <session>          # one conversation's current turn and usage
lf history --since 1d              # finished conversations and headless runs
```

```text
$ lf session list
session_21fe…   active    Repository  Agent platform craft   turn running 2m14s
0199a213-81c0…  active    raw claude  ~/src/app              turn running 41s
session_675c…   detached  LOO-472     CI under five          last turn ended 3m ago · 41.2k in (38.0k cached) · 1.1k out
```

`detached` means a held conversation is running with no terminal currently
showing it; `lf resume` reattaches.

This works for raw conversations too. Loopflow reads the tail of each provider's
own transcript read-only (`~/.claude/projects`, `~/.codex/sessions`,
`~/.pi/agent/sessions`, `~/.local/share/opencode/opencode.db`), so even plain
`claude`, `codex`, `pi`, or `opencode` sessions you started outside `lf` appear
marked `raw` with live turn status. Antigravity conversations lf launched are
read from `~/.gemini/antigravity-cli/brain`; a plain `agy` is not listed,
because its transcript does not say which directory it ran in. Pass `--no-raw` to hide them.

To adopt a raw conversation into Loopflow, pass its provider ID to `lf resume`:

```bash
lf resume 0199a213-81c0-7800…      # becomes a tracked Session you can rename, bind, and resume
```

Filter flags combine: `--wave <name>`, `--primary [task|wave|repository]`,
`--orphan`, `--on claude[:opus]`, `--search <text>`, and `--page --json` /
`--after <cursor>` for stable pagination.

Transcripts show whether a turn is running and for how long, when the
last turn ended or was interrupted, and its model and tokens. Transcripts
cannot tell whether an active turn is paused on a permission prompt or question
(providers write that only after you answer), so those read as `turn running`
unless the agent reports status directly.

## Switch agents mid-conversation

```bash
lf -a codex resume <session>       # continue the same Session on Codex
lf session new -a claude           # continue this scope on Claude
```

The Session keeps its title, Task, and history. The new harness starts from a
plain log of the conversation so far (requests, answers, and one line per tool
call) rather than a lossy summary. The switch sticks: future `lf resume` and
`lf open` calls and Desktop reopen on the new provider until you switch again.

## Steer and wake idle conversations

```bash
lf steer <session> "use the existing retry helper"   # one Session
lf --wave billing steer "invoices before refunds"    # every Session on a Wave
lf --repo . steer "freeze merges until 3pm"          # every Session in this repo
lf session steers <session>                          # pending vs. delivered
```

- **Running Sessions** receive steers at their next provider boundary (after a
  tool call or turn).
- **Idle work sessions** (Claude or Codex held by the server) wake
  automatically when a steer, Task comment, or Graph step update arrives, with no
  per-conversation watcher process required. Pi, OpenCode and Antigravity hear
  updates at their next boundary.
- **Chats** are never woken while idle; in a Task checkout they receive updates
  on their next turn.

Run one more headless turn on any Session from any directory:

```bash
lf -b resume <session> --message "now add the migration test"
```

## Name, bind, and Desktop reconnect

```bash
lf session rename <session> "Release notes"
lf session bind <session> --task LOO-12              # make it the Task's primary conversation
```

New Sessions take their title from the request, Skill, or Task (a name you set
is never overwritten by `--suggest`). Titles propagate via OSC terminal titles
and host-specific tabs (cmux, tmux).

When you open a Task, Desktop shows a spinner over any saved screen. The
terminal opens directly below the provider and status strip. You can type once
the terminal is attached and the provider is ready. After a Desktop restart,
selecting the Task reattaches to its running provider with your draft intact.

If the connection drops, the last screen stays visible. The message over the
terminal tells you how to continue.

| Message | What to do |
|---|---|
| **Reconnecting…** | Leave the pane open. If the provider is still running, your draft survives and buffered output catches up. |
| **Session unavailable** or a relay error | Click **Retry connection** to reconnect through the relay. |
| **Connection status unavailable** | Click **Refresh session** in the pane strip to reopen the viewer. |
| **Session ended** or a specific close reason | Read the reason. Use **Refresh session** to reopen the conversation. |

Retry and refresh leave Graphs running and never replay keystrokes.
See [Connect clients](server.md) for server recovery.

Interactive Sessions keep provider auth or capacity refusals visible and never
auto-retry; resolve the login or switch harnesses with `-a` on `lf resume`.
See [Native authentication](subscriptions.md).

## Reference

- **Selectors.** `resume`, `rename`, and `bind` accept a Session ID or provider
  conversation ID; `resume` also accepts a Task ID.
- **Preparing a terminal.** `lf resume <session> --prepare --json` prints the
  attach command (SSH for remote Machines) without launching a provider.
- **Headless sessions.** `lf resume` refuses headless Sessions and points you
  to `lf resume --task <ID>`.
- **History.** `lf session history <session>` prints recorded turns and events,
  including starts, resumes in a new process, forks, provider switches, and
  completions.

Every flag: [`lf session`](lf-reference.md#lf-session).

Next: [Ops](ops.md): pull requests, CI repair, stacked branches, releases,
and remote machines.
