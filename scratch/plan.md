# LOO-1097: opening a running chat elsewhere keeps it running

Brief, verbatim: "whenever i open an agy session in a different viewer Eg Rex
or Loopflow Desktop it seems to close thea gy client... is agy not being held
in the the holder the same way as other clients"

## What is happening

agy is held exactly like the other harnesses. What is not held is a **chat**,
on any harness: `lf -a agy` typed in a Rex pane runs agy in that pane
(`transport/README.md` "Chats at a terminal", #1966). Opening a running chat
anywhere else resolves to `SessionResumeIntent::TakeOver`
(`ops/human_session.rs` `resume_intent`), and `open` calls
`stop_session_agent` (SIGTERM, then SIGKILL) before starting it again. The
first pane prints "This conversation moved to Desktop: lf resume …"
(`native_session.rs:516`) and drops back to the shell.

agy hits it most because of how it gets launched. Shell history shows
`lf -a agy` and `lf resume -a agy`. In a linked checkout that no Task owns
(`manabot.mlx`, `etude.demo-googlecolab`), `lf resume` picks
`checkout_conversation`, which is the last chat, and `session_kind` reopens a
bare Session as a chat again. What the store showed on 2026-11-24
(loopflow.db, interactive agent processes, last 4 days):

| provider | chats | chats ended with no exit recorded | Task work sessions |
|---|---|---|---|
| agy | 38 | 25 | 76 |
| claude | 14 | 6 | 72 |
| codex | 6 | 6 | 0 |

Live then: two agy chats in Rex panes (`session_7f28…`, `session_e288…`,
terminal `rex:{…}`), both showing the Desktop action **Take over**. One Claude
chat in Rex showed the same action. Held agy work sessions (`89aa1683…`,
`a9e69401…`) showed **Open here** and stayed up.

## Design (chosen without review; see Passed over)

A chat that someone opens from another viewer **moves into the holder, and the
terminal it left becomes a view of it**. No client closes. Afterwards the
Session is held, so every later open adds a view (`View`), as it already does
for work sessions.

1. **The take-over lands in the holder.** In `human_session::open`, a
   `TakeOver` always delegates to a held terminal, even when the opener is a
   plain terminal where `session_kind` would answer `Chat`. Desktop already
   does this, because `LF_SERVER_STATUS` makes `session_kind` a work session.
   The change makes a Rex-to-Rex open behave like Desktop.
2. **The pane it left follows.** When a chat's provider ends because a mover
   on this Machine took its attachment (`Departure::Terminal`), the chat's
   `lf` attaches as a view of the Session's held terminal
   (`transport::attach`), instead of printing the move notice and exiting.
   The mover holds the Session's launch lock (`lock_session_process`) until
   its held terminal is registered. The follower takes that lock, reads
   `session_terminal`, then attaches, so the two never race. If no held
   terminal appears (the move was a chat-to-chat move from an older build, or
   the start failed), it prints today's notice and exits.
   `Departure::Machine` keeps its notice: the view can't follow it across
   Machines in this change.
3. **The action says what happens.** `opening()` gives `TakeOver` the label
   **Open here**, with this help text: "Moves it into Loopflow's holder; the
   terminal running it becomes a view. Unsent text there is lost."
   `resume_explanation.rs` says the same. The wire shape doesn't change;
   Swift prints the label it receives. The Swift tests and fixtures that
   assert "Take over", `swift/README.md:261` and the `SessionRecord.swift:123`
   doc comment change with it. `SessionResumeIntent::TakeOver` keeps its name
   because the mechanism is still a take-over.

Unsent input in the original pane is still lost, because the provider
restarts from its history. That is the one cost, and the help text says it.

### Sibling harnesses

The chat launch path is shared (`native_session::launch_session`; #1966 gave
each harness one path), so Claude, Codex, OpenCode, Pi and agy all get the
change. Check during implementation:
- Codex's chat: the provider is its client against a private app-server.
  Confirm the client exits on SIGTERM before the follower attaches (as
  `transport/native.rs` does).
- agy resumes with `--conversation` (`harness/antigravity.rs` `command`).
  In the holder, its first model call is a resumed start
  (`initialNumSteps > 1`) and rewrites the thread file. That is already
  covered by `a_launch_follows_the_conversation_a_person_is_in`.

## Demo

1. In a Rex pane in a scratch worktree, run `lf -a agy` and send it a message.
2. Open that Session in Loopflow Desktop (**Open here**), or run
   `lf resume SESSION` in a second Rex pane.
3. Both panes show the same agy conversation and both can type into it. The
   first pane did not drop to the shell.
4. `lf session list` shows one running agy Session with a held terminal.
   Desktop's action for it is now **Open here**, and opening it a third time
   adds another view.

## Acceptance check at gate

An integration fixture with a stand-in provider (any one harness; the path
is shared) and a `MachineServer`:
1. Start a chat in a PTY.
2. Run `lf resume SESSION` from a second PTY.
3. Assert that the first PTY's `lf` is still running and receives the held
   terminal's output, that `session_terminal` is set, that exactly one
   provider process is live, and that the first PTY never printed
   "This conversation moved to".

Command: `cargo test -p loopflow --test integration <new test name>`; the
gate step runs it. Unit tests: `opening()` labels in `human_session/tests.rs`,
`global_commands.rs:1486`.

## Absent and error states

- Server unreachable when the opener asks for a held terminal: the open
  fails with today's "Loopflow server is not running" message, before
  anything is stopped. The take-over must check for the server before
  `stop_session_agent`, so a failed open never kills the chat.
- Follower can't attach (no held terminal after the lock): it prints today's
  move notice and exits 0.
- Store unreadable in the follower: it records a gap in the log and prints
  the notice.

## Exclusions

- Chats stay in their terminal until someone opens them elsewhere. Starting
  every chat held is passed over (below).
- Cross-Machine moves keep their notice.
- `lf resume` in an unassociated checkout still selects the last chat. Once
  that chat has been opened elsewhere it is held, and the next resume views it.

## What goes

- The "Take over" label and its "that client ends" help text, in Rust,
  Swift tests and docs.
- The chat-exit notice for same-Machine terminal moves: the
  `Departure::Terminal { to: Some }` branch of `departure_notice` stays
  only as the fallback when no held terminal appears.
- Update `transport/README.md`: "Chats at a terminal", and "Using a view",
  which says that opening takes over.

## Passes

1. **Move and follow** (this pass could prove the plan wrong). Delegate the
   `TakeOver` to the holder, add the server preflight, and make the chat
   attach after its departure. Build the integration fixture first. It shows
   whether the launch lock is enough to order the follower after the held
   start, and whether a chat's `lf` can become a relay after its provider
   exits (terminal modes restored by `os_process::foreground`).
2. **Words.** Change the label and help, `resume_explanation`, the Swift
   test expectations and fixtures, the READMEs and
   `docs/` mentions of take-over.

## Passed over

- **Every interactive launch held** (chats become relays from the start).
  It removes the chat kind for terminals, so there is one concept and opening
  always adds a view. It reverses #1966's "closing the terminal ends a chat"
  and the open agent-in-terminal direction (agent runs in the pane, Rex shows
  its icon). That is too large to decide headless. The chosen design moves a
  conversation into the holder only when a second viewer asks for it.
- **Refusing or warning before a take-over.** It adds a guard and doesn't
  give anyone the second view they asked for.
