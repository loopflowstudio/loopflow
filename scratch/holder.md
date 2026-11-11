# LOO-824: hold managed Sessions' PTYs beside the server

Stacked on LOO-823 (#1877); LOO-821 (#1879) and LOO-819 (#1875) are on main.
The direction is `docs/dev/server-first.md`; the Task brief's "Decided, do not
reopen" list is not repeated here. Scratch in git: LOO-819 `e65ddaed7:scratch/`,
LOO-821 `8044329e6:scratch/`, LOO-823 `origin/jack/loo-823-…:scratch/process-leaks.md`.
This file's fuller history (explore findings, build log, pass counts):
`8c5e05e50:scratch/holder.md`.

## Outcome

Ten Task Sessions open: `ps` shows one `lf server run`, one `lf holder run`,
ten agents and no `lf session serve --terminal`. `lf install` mid-turn: the
turn keeps streaming, Desktop panes say Reconnecting… until the next
server answers, and `lf resume --task` terminals draw nothing meanwhile and
continue from their PTY position. `kill` the server: agents keep running
(and keep writing into the holder's ring); launchd restarts it; views return.

## Said, relevant here

Jack Heart, LOO-819 conversation (full set at `e65ddaed7:scratch/server-model.md`):

- On several old holders staying alive: "im ok with that"
- Emulator in the server, holder keeps only raw bytes: "Yea the second"
- Size: "Owner sets the size. I would say that we switch owners on viewer open close; any new viewer becomes the owner riht away?" (later Task, not this cut)

## Probes, November 10 2026

**Holder survives the server's service (macOS).** A launchd agent in
`gui/$UID` whose child called `setsid()` and slept. `launchctl bootout`: the
setsid child survived both with and without `AbandonProcessGroup`; with it,
the job's own non-setsid child survived too (reparented to 1). The shipped
`com.loopflow.server.plist` already sets `AbandonProcessGroup`; the systemd
unit already has `KillMode=process`. Not probed: SMAppService unregister
(same launchd job underneath), Linux (left to the Linux QA rental at gate).
Logout and reboot end holders, as they end owners today.

**Size nudge repaints the whole visible screen.** Harness: agent idle in a
40×120 PTY, full stream into a pyte emulator; then resize to 119 cols (and
separately 39 rows), 0.4 s, back to 40×120, 3 s; a fresh emulator fed only the
post-nudge bytes compared row by row with the full one.

| Agent | Screen | Post-nudge bytes | Non-blank rows matched |
| --- | --- | ---: | ---: |
| Claude Code 2.1.322 | idle start | 1,631 | 9 / 9 |
| Claude Code 2.1.322 | after a 30-line answer | 1,519 | 36 / 36 |
| codex-cli 0.167.0 | idle start | 3,995 | 23 / 23 |
| codex-cli 0.167.0 | after a 30-line answer | 1,718 | 34 / 34 |

Cursor position matched in every case. Scrollback above the screen is not
repainted (both print history inline), and pyte does not model terminal
modes, so whether either re-sends bracketed paste, focus or Kitty keyboard
modes on resize is unknown. Script: re-creatable from this description.

Superseded kickoff candidate A: `06532757c:scratch/holder.md`.

## Risks and answers

| Risk | Answer |
| --- | --- |
| Service manager kills the holder with the server | Probe: setsid child survives `bootout`; plist and unit already abandon children |
| Second holder steals the socket | Kernel lock; unlink only under it; spawn is idempotent |
| Server's launchd environment lacks the person's PATH/auth env | `start` carries the caller's complete environment; served `task.run` starts use the served operation's Graph environment |
| Output while server is down blocks the agent | Holder drains into the ring; ring loss is only scrollback |
| Lost modes after any server restart | No snapshot; ring replay plus nudge. Modes set before the ring's start are unverified (slice 0 probe, not run) |
| One panicking owner loop takes the server down | Each loop thread runs under `catch_unwind`; the Session stays held and the loop restarts from the ring |
| 95 loops in one process | Threads idling in `poll`. Gate: `ps` RSS/CPU against 95 × `session serve` (4.4 GB, 55%) |

## Forbidden

- A managed launch that silently falls back to a local owner.
- Unlinking a holder socket without its lock.
- The holder reading or writing the store, or interpreting Sessions.
- A server restart ending a Session because a holder connection dropped.

## Acceptance at gate (headless)

```sh
uv run python scripts/test.py --reuse-passing --rust-filter 'test(holder::) | test(transport::) | test(server::)'
```

Integration test `held_session_survives_server_restart` (in
`tests/transport_tests.rs` beside the other server fixtures, isolated
`LF_HOME`, fake agent script printing a counter every 100 ms):

1. Start a server fixture; `session.start` the fake agent; attach a viewer;
   see counter lines. `ps` of the fixture's process group: one server, one
   holder, the agent, no `session serve`.
2. SIGTERM the server; agent pid still alive; counter keeps advancing in the
   holder (`read` position grows).
3. Restart the server; viewer reconnects; its screen contains the newest
   counter and a counter value printed while the server was down.
4. SIGKILL the server; the agent exits 3; restart: Session ended with code 3,
   `<pty>/` gone; the open view prints `claude exited with 3.` (`native_exit::exit_line`,
   from its Session's provider) and exits 3.
5. `held_terminals_list_one_holder_process`: `lf ps --json` lists one holder
   LfProcess and no owner row per held terminal.

Linux: same test on the RunPod rental with the user unit's `KillMode=process`.

## Done when

`ps` with ten open Task Sessions shows one server, one holder, ten agents and
no `session serve`; an `lf install` mid-turn and a server kill both leave the
turn running and every view repainted; a Session whose agent exits while the
server is down reads ended with its real code once the server is back.

## Explore — November 10, 2026

[explore.html](explore.html). Jack Heart: "what would rex do? what do t3code /
paradev / paseo do?" (Rex and Paseo keep screen state with the long-lived
terminal owner; Paradev's daemon replays a 64 KiB ring; T3's provider runtimes
end on server shutdown.) Then: "Please rrewrite the explore.html to help me
understand what will move things most in the direction of: - High performance,
low overhead, maximum scalabiltiy for a lot of concurrent work - Best
isolation of responsibiliies, least duplicative with with tools like Rex -
Best overall UX for a loopflow desktop user"

`ps` at 18:29 that day: 126 `session serve`, median 36 MB RSS (max 76), 70%
CPU summed; all lf about 19.6 GB, of which `session serve` 4.4 GB.

Jack Heart: "1b seems right" and "unsure on 3a vs 3b. Why did I prefer 3a
before? What would you recommended". The LOO-819 notes keep only "Yea the
second", not a reason.

## Direction after explore — November 10, 2026

Jack Heart: "1b seems right", then "ok 3a 4a". Question 2 (plain
`lf resume SESSION`) has no answer yet; the disk journal (1c) was not taken up.

This replaces "Kickoff candidate A" where they differ. The holder section,
the managed rule, Risks, Forbidden and the explore findings (terminal
identity, start reconciliation, exit receipts) stand.

Built as planned: terminal registry keyed by `TerminalId` (1b), one
libghostty-vt terminal per held terminal in the server (3a), view positions in
PTY bytes resumed from the holder's ring (4a); after a server restart the
emulator is rebuilt by ring replay plus one size nudge, no snapshot file.
Slice 0 (mode probe through production Ghostty) is not run.

Jack Heart, on question 2 after asking what "ended" means: "i think i want 2a"
(plain `lf resume SESSION` of an ended Task/Wave Session starts managed).

### Positions, queries and view drawing — November 10, 2026

Three gaps were raised from a pasted review: positions need an identity,
who answers the agent's terminal queries, and what a view draws itself.
Jack Heart: "(1) no opinion", "(2) pasted proposal works", "(3) i dot know
that we want to do any overlay for raw lf".

- **Position identity.** A `TerminalId` names one PTY and is never reused; a
  process started by resuming an ended Session gets a new one. A `read` or
  resume naming an id the holder does not hold is an error, never an answer
  from byte 0. Test both.
- **Queries.** (First proposal, revised below.) The server's emulator
  answers every query; view replies are dropped before the PTY.
- **View drawing.** The plain-terminal relay draws nothing of its own into
  the screen while reconnecting, so its position stays exact and resume
  needs no repaint. Desktop keeps its overlay, which is outside the stream.
  Any text the relay still must print into the terminal (for example on
  giving up) is followed by a refresh, not a position resume.

Jack Heart, after the above: "that said i am ok with deferring 2 to one of
hte other prpojects if you think thats better"

**Queries, revised: deferred.** This Task keeps today's rule: the emulator
answers only while no view is attached, and the controlling view's terminal
answers otherwise. The one new case 4a creates is a view catching up from an
old position, whose terminal answers queries already answered; the relay
discards terminal input until it has caught up (`discard_reconnect_input`
already does this while reconnecting). "The server's emulator answers every
query and view replies are dropped" moves to the every-view-types piece,
where several typing views make it necessary; telling a query reply from a
keystroke in a view's input, and what color queries return, are designed
there. `docs/dev/server-first.md` records it under that piece.

All three built: `holder::tests::reading_a_terminal_the_holder_does_not_hold_is_an_error`,
`transport::registry::tests::a_resumed_session_gets_a_new_terminal_and_the_old_position_is_refused`,
`transport::tests::a_catching_up_views_terminal_answers_never_reach_the_agent`,
`transport::tests::a_held_screen_the_relay_drew_into_is_refreshed_not_continued`.

## Implementation — November 10, 2026

What is built is in `transport/README.md` (held terminals, Views) and
`server/README.md`. Kept here, because it isn't in the code:

- Holder startup hashes its executable: the debug binary took over 2 s to
  print its ready line. `Client::ensure` waits up to 20 s.
- A rebuilt screen answered the queries replayed from the ring, sending stale
  replies into the agent's PTY after every restart; `Hosted::take_up` marks
  the replay (`queries_replayed_from_the_ring_are_not_answered_again`).
- After a restart a held pane read `connected` with `native_ready:false`
  forever (the launcher announces `NativeStarted` once, then execs); fixed by
  `native-client.json` beside the terminal.
- A launcher that waits beside its provider (non-exec path) sends
  `NativeEnded` to the server it announced to; after a restart that end is
  lost and the exit text is replaced at the child's exit. Untested.
- Revision 5 has no revision-4 fallback: a Desktop or CLI on either side of
  the bump answers `incompatible` until both are installed.

Not as planned, open without a slot: the gate test's `ps` step checks the
agent's parent is `lf holder run`, not a whole-group count; a superseded
build in a holder is never moved onto the installed lf by `lf resume`.

## Concept review — November 10–11, 2026

No snapshot on graceful stop: open views resume by position; ring replay plus
one nudge serves cold opens. "No per-terminal socket" became: server threads
do not grow with held terminals (`server_threads_do_not_grow_with_held_terminals`),
one holder LfProcess per build (`held_terminals_list_one_holder_process`).

## QA — November 11, 2026

[qa.html](qa.html). A holder unanswered at startup is pending, not gone:
concept review answer "settled, retry". Probe on this Mac
(`kern.ipc.somaxconn` 128): a unix listener that never accepts refused the
129th connect with ECONNREFUSED, so a refused socket cannot mean gone; the
holder's kernel lock decides (`Place::claimed`). Linux queue behavior not
probed.

## Concept review with a person — November 11, 2026

[concept-review.html](concept-review.html). Plain terminal title reading
Reconnecting… while the server is away: "not sure"; nothing drawn, as built.
Unbuilt: one place listing the conversations a killed holder took.

### Second round, against LOO-821/822/823 — November 11, 2026

Jack Heart: "can you talk to 821, 822, 823 and read how their designs have
progressed and the regenerate the concept-review".

Jack Heart, after asking "what is managed vs eld?": "managed shoudl = held"

One predicate decides holding and hooks, `WorkSource::is_managed`, plus a
terminal viewing the launch (piped and `-b` stay unheld).

Jack Heart, on `lf resume <session>` of an `lf implement` run typed in a Task
checkout: "bare is correct here". Matches the steer above (not held).

Jack Heart, on "a Desktop conversation with no Task or Wave": "no such
thing? unless you mean soemthing that happens inside a shell in desktop?",
then "i think teh right hting to do there is to hold shells probably".

Then: "if we want to do that. dont need to do it yet". Shells stay on their
owner in LOO-824; holding them is later work, not filed.
Then: "actually we probably do watn to do that for mobile anyway".
Then: "we should amke the desktop hold the pty for all shells in desktop/mobile and throw them away when we throw away task sessions". Filed as LOO-830.

Jack Heart, on a managed launch with the server down (LOO-821 starts it, then fails; LOO-824 fails at once): "821 is correct here". Matches the "managed should = held" steer: require_server, ServerMissing goes.

Jack Heart, on how long a held terminal lives: "823s rule is correct". Held terminals run until their Task is done or abandoned, their Wave or repository conversation closes, or their agent exits. No idle grace for held terminals; sync onto LOO-823 once it lands.

Jack Heart, on the three still-open items: server reads status for every
held terminal: "do it now"; plain terminal title reading Reconnecting…:
"not sure"; LOO-830: "the servers holder should hold the pty".

Built: the server records each held terminal's OSC 7501 Program Status as the
Session's, view or not; Desktop's `session.status` for a held Session is
ignored.

Jack Heart, in the LOO-821 conversation (comment 778081f8): "no. in 824 i
explained that there is no such thing other than one you start through a
shell. shells in desktop should be managed by the holder as well, but they
should not be managed (and their pty holding should be more generic than the
lf session one)".

Every managed launch is held; the holder will also hold Desktop shells, which
stay bare (LOO-830). Shells stay on their owner in LOO-824.

## Implementation — November 11, 2026

Built: the `held.sock` revision (another revision's view execs the installed
`lf`; `--moved` and `--session-json` are not carried), held Program Status,
`require_server` for managed launches.

## Concept review, headless — November 11, 2026 (third)

All six steers are built and recorded above. One mismatch is left in the
managed rule.

Open then: is the repository conversation managed? It recorded no
WorkSource, so `is_managed` called it bare while `managed()` held it from a
Desktop pane through `LF_SERVER`. Options offered: (a) managed, one rule
wherever opened; (b) bare, held from Desktop only. Text:
`8bfcccdfc:scratch/holder.md`.

Jack Heart, in the LOO-820 conversation (comment a6a8449e): "repo and wave sessions should be managed. you can tell 84 that". The repository primary records Declared, so is_managed holds it; managed() drops its LF_SERVER branch (held = task_run || is_managed).
Then: "the repo is really secretly just the a wave".
Then: "i just meant concpetually. whether we should give repo a wave project and stuff we dont need to wrory about". Not filed.

Built: `repository_session` records `Declared`. Repository primaries recorded
earlier with no WorkSource stay bare.

## QA — November 11, 2026 (second pass)

[qa.html](qa.html): ten Task Sessions on one holder, unviewed Program Status
reading `available`, revision change, SIGKILL and `launchctl kickstart -k`
restarts with contiguous output, exits settled by the next server.

Jack Heart: "can you stack ontop of 821".

## Stack merges — November 11, 2026

LOO-821 merged twice (`8044329e6`, `e3a86b13d`), then landed on main as a
squash (#1879, `763da5c92`). Fresh launches hold by the command
(`task_run || cli.names_work()`); reopening by id alone reads the row
(`transport::held`, `require_server_for`, `WorkSource::is_managed`). Jack
Heart: "i think (1a) is right, so double coverage for now". Desktop panes
already name their Task or Wave on reopen (`SessionsView.swift`
`prepareSession(work:)`, from LOO-821); a Session link names none and the
row read covers it.

Merged origin/main (`c5cf93194`): conflicts re-merged against LOO-821's last
merged tip `e3a86b13d` so the squash could not undo this branch; two files
(`agent/session.rs`, `lf/commands/session.rs`) had kept main's copies of code
this branch deleted. Merged LOO-823 (`8c5e05e50`, its tip `4b7531508`):

- Owners and held terminals record viewing only (`transport/lifetime.rs`, one
  thread per host for every execution it serves). Task cleanup ends a done
  Task's conversations after the viewing grace and an abandoned Task's
  unviewed ones: an owner by signal after its notice, a held terminal by the
  notice itself (`close_notice(&Host)`; the server retires it).
- Held arms added to `task_close_candidates`, `pending_retirements`,
  `task_session_hosts` and `task_viewer_holds` (`OwnerHold.owner: Host`).
- `held_terminals.native_input` dropped (draft migration edited in place);
  `host_repository` and the held special cases in lifetime are gone.

Checks: `--lib transport::lifetime:: ops::task::retirement::tests::
store::sqlite::task_retirement::` 20 ok; `transport_tests` held, cleanup and
upgrade filters 22 ok, incl. `task_cleanup_ends_an_abandoned_tasks_held_terminal`;
`--lib transport:: holder:: ops::human_session:: store::migrations::
store::sqlite::sessions desktop::shells` 258 ok; clippy `--all-targets` and
`documented_commands::` clean. Left for gate: the full affected suites.

Compress (Nov 11): `cargo test -p loopflow --lib holder::` 10 passed; `cargo clippy --all-targets -D warnings` clean.
