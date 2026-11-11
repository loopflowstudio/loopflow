# LOO-824: hold managed Sessions' PTYs beside the server

Stacked on LOO-819 (#1875). The direction is `docs/dev/server-first.md`; the
Task brief's "Decided, do not reopen" list is not repeated here. LOO-819's
scratch (quotes, measurements, receipts) is in git at `e65ddaed7:scratch/`.

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
| Server's launchd environment lacks the person's PATH/auth env | `start` carries the caller's complete environment; server-initiated starts (`primary.rs` under served `task.run`) use the environment the served operation already uses for Graph launches; implement confirms which |
| Output while server is down blocks the agent | Holder drains into the ring; ring loss is only scrollback |
| Lost modes after any server restart | No snapshot; ring replay plus nudge. Modes set before the ring's start are unverified (slice 0 probe, not run) |
| One panicking owner loop takes the server down | Each loop thread runs under `catch_unwind`; the Session stays held and the loop restarts from the ring |
| 95 loops in one process | Today's loops idle in `poll`; threads, not processes. Measure `ps` RSS/CPU at gate against today's 95 × `session serve` (4.4 GB, 55%) |

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
   `<pty>/` gone; the open view prints `The agent exited with 3.` and exits 3.
5. `held_terminals_list_one_holder_process`: `lf ps --json` lists one holder
   LfProcess and no owner row per held terminal.

Linux: same test on the RunPod rental with the user unit's `KillMode=process`.

## Done when

`ps` with ten open Task Sessions shows one server, one holder, ten agents and
no `session serve`; an `lf install` mid-turn and a server kill both leave the
turn running and every view repainted; a Session whose agent exits while the
server is down reads ended with its real code once the server is back.

## Explore — November 10, 2026

Jack Heart: "$explore"

Review [explore.html](explore.html). These are proposed implementations of
the same decided ownership model: one holder per build, emulator in the
Machine server, a raw ring in the holder, single-controller views for this
Task. Bare execution and every-view typing remain separate work. The
kickoff probes above are recorded observations, not tests of these visions.

### Findings that change the kickoff candidate

- `docs/dev/reference-models.md:139` defines an LfProcess as an actual lf
  process. Ten owner rows with the holder's PID do not describe ten processes.
  All visions need a held-terminal identity separate from the holder's one
  LfProcess and from each actual launcher/AgentProcess. Do not assign a
  fabricated process to a server thread. Existing launcher ancestry and
  capture attachment remain real process records.
- `transport/snapshot.rs::stream` rejects a different Ghostty revision. A
  graceful upgrade can therefore require ring replay and a repaint too.
  Snapshot position and ring continuity must agree before replay; a stale
  snapshot cannot bridge overwritten output. Exact modes after a gap remain
  unproven by the pyte probe.
- `harness/native_exit.rs::settle` returns a provider-facing outcome even
  when store/capture writes fail. Calling it is not confirmation of recorded
  settlement. Keep the exit receipt until all required recording is confirmed;
  retry recording without replaying a launch or closing a newer attachment.
- An unanswered `start` may have started a child. Give it a stable terminal
  id and request identity; reconcile via holder inventory before any retry.
  Reusing an id with different launch content is an error. Do not infer death
  from a missed reply. A successful list lacking the PTY but no exit receipt
  is an unknown outcome, not a fabricated exit code.

These are correctness requirements for every candidate, not optional extras
to trade against delivery time. Holder restart does not resurrect a process.

### A, B, C — superseded

A moved the owner loops into the server; B modeled the held terminal as a
registry keyed by `TerminalId`; C added a disk recovery journal. B was taken
(1b, below); full text at `a18d6a915:scratch/holder.md`.

### Competitor comparison — November 10, 2026

Jack Heart: "what would rex do? what do t3code / paradev / paseo do?"
Documentation reads at pinned commits (links: `8bfcccdfc:scratch/holder.md`).
Rex and Paseo keep screen state with the long-lived terminal owner (Paseo's
worker: PTYs plus headless xterm, 10,000 rows; it may close workers two
protocol versions behind, not to be copied). Paradev's PTY daemon: in-memory
replay, fd handoff on upgrade, 64 KiB ring (512 KiB via DaemonSupervisor).
T3: server-owned node-pty with bounded history; provider runtimes end on
server shutdown. A surviving emulator in the holder is an axis not taken here.

### Three goals — November 10, 2026

Jack Heart: "Please rrewrite the explore.html to help me understand what will
move things most in the direction of: - High performance, low overhead,
maximum scalabiltiy for a lot of concurrent work - Best isolation of
responsibiliies, least duplicative with with tools like Rex - Best overall UX
for a loopflow desktop user"

[explore.html](explore.html) is rewritten around those goals. Nothing is
selected. Observations it rests on:

- `ps` at 18:29: 126 `session serve`, median 36 MB RSS (max 76), 70% CPU
  summed, 30 of them at or above 1% and 69 at or above 0.1%. All lf: about
  19.6 GB, of which `session serve` 4.4, `__context-block` 8.3 (62 processes),
  `observe-status` 2.4 (113), `__codex-observe` 2.4 (42, 115% CPU). The
  holder removes the 4.4; the rest belongs to other server-first pieces.
- A viewer's resume position counts its own stream (snapshot + out frames),
  kept in a 1 MiB spool per viewer inside the owner (`transport.rs` `Spool`,
  `wire.rs` header). A restarted server has no spool, so every open view
  refreshes from the rebuilt emulator. Positions in PTY bytes, answered from
  the holder's ring, would let an open pane continue across a server restart
  with no repaint. Proposed on the page as lever 3; not in the slices above.
- A joining view is painted at most 2,000 rows / 768 KiB of history
  (`transport/terminal.rs`). A 2 MiB raw ring usually holds more than that,
  so what a thin holder puts at risk after a restart is terminal state set
  before the ring's start (modes, alternate screen, Kitty keyboard stack,
  program colors), more than rows.
- The owner's emulator answers the agent's cursor, device and mode queries
  while no view is attached (`transport/README.md`). Some emulator, or a
  responder, has to exist headless wherever no view is open.

Page's reading: 1b + emulator in the server (3a) + PTY-position resume (4a),
no journal; before implement, repeat the resize probe through the production
Ghostty emulator to see whether Claude and Codex re-send modes. If they do
not, 3b (emulator in the holder) is what fixes cold opens, at the cost of the
holder covering what Rex already does.

Jack Heart, after the three-goals page: "1b seems right" and "unsure on 3a
vs 3b. Why did I prefer 3a before? What would you recommended"

The LOO-819 notes keep only the reply to the emulator question ("Yea the
second"), not a reason; the original exchange was not found under
`~/.lf/runs`. The reasoning in `docs/dev/server-first.md` is that a holder is
never restarted for an upgrade, so its protocol stays small.

## Direction after explore — November 10, 2026

Jack Heart: "1b seems right", then "ok 3a 4a". Question 2 (plain
`lf resume SESSION`) has no answer yet; the disk journal (1c) was not taken up.

This replaces "Kickoff candidate A" where they differ. The holder section,
the managed rule, Risks, Forbidden and the explore findings (terminal
identity, start reconciliation, exit receipts) stand.

**What is built**

- **Terminal registry (1b).** The server keeps held terminals keyed by a
  `TerminalId`: Session → TerminalId → holder (build, pid) + PTY + child
  AgentProcess. No LfProcess row stands for a server thread; the holder has
  one LfProcess. One server event loop reads holders and views; per-terminal
  queues are bounded so a noisy agent or a stalled view delays nobody else.
  No private socket or thread per terminal. `session.start` returns a
  `terminal_id`; managed attach resolves it in the registry. The
  `endpoint(owner)` route stays only for `Local` owners.
- **Emulator in the server (3a).** One libghostty-vt terminal per held
  terminal, as the owner has today: paints joining views, answers the
  agent's queries while no view is attached. The holder never parses output.
- **PTY positions (4a).** A view's stream position is the holder's absolute
  PTY byte count. A snapshot frame carries the position it covers; `out`
  frames follow from there. Resume is `{terminal_id, from}`: the server
  serves from the ring, or answers with a fresh snapshot when `from` is
  older than `ring_start`. The per-viewer `Spool` is deleted for held
  terminals. Events that ride the stream as APC sequences are not PTY bytes;
  they move to their own frame tag so positions stay exact (implement
  confirms nothing depends on their in-stream order beyond the title
  update).
- After a server restart, open views resume by position with no repaint;
  the emulator is rebuilt by ring replay plus one size nudge (no snapshot
  file; see the concept review's second pass).

**Slices**

0. Mode probe: the kickoff resize harness with the production Ghostty
   emulator in place of pyte, Claude and Codex, idle and after an answer;
   compare bracketed paste, focus, Kitty keyboard and mouse modes between
   the full-stream emulator and the post-nudge one. Result goes here. It
   does not gate slices 1–3; it says how cold opens after a restart behave.
1. Holder: as kickoff slice 1.
2. Registry and event loop; `session.start`; the managed rule and its
   callers; emulator per terminal; views attached through the registry with
   today's single-controller rules.
3. PTY positions in the viewer stream; resume from the ring; server restart
   rebuild; offline settlement with retained exit receipts.
4. Docs, revision bump with the other pieces, `server_schema.json`,
   `tests/fixtures/dto/` for the changed wire types.

**Acceptance, added to the gate test**

- After the server is SIGTERMed and restarted, the attached viewer receives
  no snapshot frame: its next frame is `out` at the position it held, and
  the bytes between are the ones printed while the server was down.
- A viewer whose position is older than `ring_start` receives one snapshot
  whose position is inside the ring.
- Server thread count does not grow with held terminals (sockets do: one
  holder connection per terminal and per view):
  `server_threads_do_not_grow_with_held_terminals`.
- `lf ps --json` lists one holder LfProcess per build and no owner row per
  held terminal.

Demo: ten Task Sessions in Desktop, `lf install` mid-turn: every open pane
pauses and continues without redrawing; a pane opened afterward shows its
screen and history; `ps` shows one server, one holder per build, ten agents.

Jack Heart, on question 2 after asking what "ended" means: "i think i want 2a"

Plain `lf resume SESSION` of an ended Session bound to a Task or Wave starts
its new process managed, as the managed-rule table above already has it.

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

Status: all three built.

- Identity: `holder::tests::reading_a_terminal_the_holder_does_not_hold_is_an_error`
  (`read`) and
  `transport::registry::tests::a_resumed_session_gets_a_new_terminal_and_the_old_position_is_refused`
  (a resumed Session gets a new `TerminalId`; attaching the ended one is
  `NotFound`; the holder refuses a read from the old position).
- Catch-up discard: the relay drops terminal input until `live`, then
  writes `CSI 5 n` and drops input until the terminal's `CSI 0 n` (1 s cap);
  `transport::tests::a_catching_up_views_terminal_answers_never_reach_the_agent`.
- View drawing: `reconnect_held` draws nothing; every line the relay prints
  into a held screen calls `RelayOutput::drew`, so the next recovery attaches
  fresh. Giving up exits the relay. Desktop draws only through its overlay.
  `transport::tests::a_held_screen_the_relay_drew_into_is_refreshed_not_continued`.
- `cargo test -p loopflow --lib -- transport::registry::tests:: holder::tests:: transport::tests::a_held_screen transport::tests::a_catching_up`:
  21 passed.

## Implementation — November 10, 2026

**Built (commit 463957a85): the holder.** `rust/loopflow/src/holder.rs`,
`holder/wire.rs`, `holder/client.rs`, `lf holder run` (hidden, dispatched in
`main` before any store or journal). `cargo test -p loopflow --lib holder::`:
10 passed. Differences from the plan above, now what the code does:

- Requests and answers are one JSON line each; a `read` stream is frames
  `o` out, `g` gap, `m` mode, `x` exit. `write` is raw bytes after the answer.
- **`m` mode frames and a `restore` request are new.** The owner's
  `NativeClient` (`transport/native.rs`) tells a provider's exit text from its
  screen by `tcgetattr` on the PTY at each read, and restores cooked modes
  when a client dies raw. The server has no PTY fd, so the holder samples
  ICANON at each read, keeps the transitions by position, and frames them.
- **`forget {terminal}` is new**: an exited terminal's ring stays for readers
  until the server has settled its exit.
- `read` from a position past `written`, or of an unknown terminal, is an
  error line; the connection stays in request mode.
- `resize` also signals the foreground group (same-size resize repaints), as
  the owner does.
- Holder startup hashes its executable: the debug binary took over 2 s to
  print its ready line. `Client::ensure` waits up to 20 s.

**Learned reading the owner (`transport.rs` `serve`), for the slices below:**

- A viewer's stream is not PTY bytes. It also carries lf's own reports (OSC
  title, status, directory from `output.take_reports`), native-exit
  replacement text, settled lines and the APC events. Under PTY positions
  those travel in a frame the viewer writes but does not count (`aside`), and
  a `position` frame says where a snapshot or a dropped stretch leaves it.
- A position resume cannot replay events or asides sent while the view was
  away. The server sends the current state again (size, owner, control,
  return, directory, title and status, child exit), as a fresh attach does
  without the snapshot. After a server restart there is no viewer record, so
  resume is "attach from byte N" and answers a new viewer identity.
- A resuming view's bytes come from the holder's ring through a catch-up
  `read` connection, not from a second ring in the server.
- The store keys everything about a transport on the owner's LfProcess row:
  `agent_sessions.transport_lf_process_id REFERENCES processes(id)`,
  `processes.native_input` (input retention), viewer holds
  (`task_retirement.rs`), `owner_retention`, `idle_retention`,
  `owner_sessions`, `owner_task_abandoned`, `owner_retiring_tasks`,
  `lifetime.rs`, `snapshot::leave`, `terminal_title`, `rollover.rs`: 32 SQL
  sites. A `TerminalId` that is not an LfProcess needs a `terminals` table
  and each of those read through it. The handoff the child reads
  (`LF_TRANSPORT_OWNER`, `transport::current()`) carries an LfProcessId too.
- The lifetime policy runs one watcher thread per owner (`lifetime::spawn`);
  the registry needs one for all terminals.

**Built since** (commits a8a4223f8 and the one after):

- `transport/hosted.rs` `Hosted` + `Child::{Local, Held}`; `transport/held.rs`
  `Source` reads a terminal from a PTY position, reconnects with backoff and
  settles from `exit.json` when the holder is gone.
- `TerminalId`; store names `held_terminals` / `held_terminal_id`
  (`processes.terminal_id` already means Desktop pane placement). Store reads
  take `Transport::{Owner, Terminal}`; `transport::Host` is the same choice in
  the transport, the child handoff carries it (`LF_TRANSPORT_HELD`), and
  `session_transport` answers it.
- `transport/registry.rs`: one thread hosts every held terminal behind
  `<private dir>/held.sock` (first line `{"terminal":ID}`, answer
  `{"hosted":bool}`, then the viewer protocol). `meta.json`, lost-start
  reconciliation, re-hosting from holder `list` with ring replay and a
  one-column nudge, settlement with the directory as receipt.
- Server: `session.start`; `session.attach` prefers a held terminal and takes
  `terminal_id`; tickets and `bridged` carry an owner or a terminal.
- Relay by `Host`: `lf session attach --terminal ID`; a held view reconnects
  through the entrance by itself, draws nothing, discards input meanwhile.
- `transport::managed` in `delegate` (rule since replaced; see the last
  Implementation section). `start_held` is the client of `session.start`;
  the Task primary conversation and `restore` use it.

Checks then: `resumed_session_completes_turns_after_build_replacement_and_viewer_return`
passes with real binaries (Task conversation moved to a held terminal, server
killed and restarted mid-conversation, next turn typed by a new view, exit
settled). Pass counts from this and later passes are in git history.

Found on the way: frames arriving with the holder's `reading` answer were
left undecoded until more output came (fixed in `Source::read`). A test
holder idles out in 2 s, too short beside a slow store open; fixtures set
`holder::TEST_IDLE` on the holder thread.

**Built: position resume (4a).** Viewer frames `position` (0x05) and
`aside` (0x06), `attach.from`, `live` event; `transport/held.rs` `CatchUp`
reads the holder's ring on its own connection up to the live position;
`Hosted::catch_up` resends state (`Output::state`) then `live`. What it
does is in `transport/README.md` "Views".

- Found on the way: a rebuilt screen answered the queries in the replayed
  ring, sending stale replies into the agent's PTY after every restart.
  `Hosted::take_up` now marks the replay; nothing answers it
  (`queries_replayed_from_the_ring_are_not_answered_again`, fails without).
- A catching-up controller does not count as the answering view: the
  retained screen answers live queries until it is live.
- A view continuing from a position gets no input hint again.

Checks then: `held_session_survives_server_restart` fails showing the
repaint with continuation disabled. `resume_takes_over_an_owner_from_another_build`
and `unanswering_superseded_owner_preserves_uncertain_draft_and_pending_turn`
(owner path) fail in full `transport_tests` runs under load and pass alone;
gate reruns them.

- Revision 5 (Rust `REVISION`/`MIN_REVISION`, Swift `ServerProtocol`,
  fixtures, server README); Swift `ServerMessage.bridged` decodes a
  `ServerBridge` (owner or terminal) from both required nullable keys, with
  `bridged_terminal.json`; Swift mirrors `session.start` so
  `everyFixtureIsMirrored` holds. Desktop never speaks the viewer protocol
  itself: panes run `lf session resume`, so `position`/`aside` frames reach
  only the Rust relay. Desktop's own `session.attach` encodes a null
  `terminal_id` and nothing reads its `bridged`.
- `lifetime::Watcher` (one thread judging every owner a host watches) and
  `terminal_title::TitleReader` (one Session-name reader), bundled as
  `hosted::Watchers`; the registry owns one set for every held terminal, an
  owner process its own. Activity is read once per 2 s poll.
- `exit-record.json` beside a held terminal: written on hand-off, removed on
  drop or once settled; read back on take-up; a server-down exit settles it
  (as SIGKILL when nobody saw the end) before `end_terminal`.
- Checks: `swift test --filter 'ServerProtocol|ServerClient'` passes after
  rebuilding `target/debug/lf`; `cargo test -p loopflow --lib transport::`
  73 passed (incl. `one_watcher_ends_each_quiet_owner_and_keeps_the_viewed_one`,
  `an_exit_record_kept_while_no_server_ran_is_settled_by_the_next_one`,
  `a_server_taking_up_a_terminal_settles_the_exit_record_kept_beside_it`,
  both failing with the record unread); `server::` 59 passed;
  `held_session_survives_server_restart` passes; `documented_commands::`
  passes; clippy clean.

- Pane placement: `session.start` and holder `Start` carry `placement`
  (the pane `current_terminal_id` validated on the caller's terminal); the
  holder sets `LF_TERMINAL_ID` and the held slave's tty name for the child.
  `held_desktop_launch_links_its_session_to_the_pane` (fails with the holder
  lines disabled).
- Desktop wording already existed (`ServerConnectionReading.title`,
  "Reconnecting… Input paused; last screen may be stale."), driven by the
  relay's `reconnecting` reading. Found on the way: after a restart a held
  pane read `connected` with `native_ready:false` forever, so Desktop showed
  Opening… and refused keys. The launcher announces `NativeStarted` once and
  execs its provider (CLOEXEC closes the socket), so only the server can keep
  it: `native-client.json` beside the terminal, removed when the client ends,
  read back in `Hosted::keep_in`. `a_held_pane_reads_reconnecting_until_the_next_server_answers`
  (fails without). Swift `ServerConnectionTests` asserts the title.
- A launcher that waits beside its provider (non-exec path) still sends
  `NativeEnded` to the server it announced to; after a restart that end is
  lost and the exit text is replaced at the child's exit instead. Untested.
- Checks: `cargo test -p loopflow --lib -- transport:: holder:: server::schema::tests:: server::wire::tests::`
  92 passed; `--test transport_tests` 110/111, `resume_takes_over_an_owner_from_another_build`
  (owner path, untouched) failed once in the full run and passed alone;
  `documented_commands::` passes; clippy clean. Swift tests: gate.

**Remaining, in order:**

1. Slice 0 mode probe (resize nudge through production Ghostty with real
   Claude and Codex): not run; does not block gate.
2. Revision 5 has no revision-4 fallback: a Desktop or CLI on either side
   of the bump answers `incompatible` until both are installed.

**Not as planned, open without a slot:**

- The gate test's `ps` step checks the agent's parent is `lf holder run`,
  not a whole-process-group count.
- `attaches` for a held terminal is "its child runs": a superseded build in
  a holder is never moved onto the installed lf by `lf resume`.
- `lf ship` interactive steps and Wave session commands are held only
  through the Session rule (Task/Wave bound) or `LF_SERVER`; no separate
  call site was needed. Desktop panes with `LF_SERVER` are held; Desktop
  itself was not run against this build.

Gate/CI: Swift and Desktop suites; the Linux RunPod run of
`held_session_survives_server_restart` with the user unit's `KillMode=process`.

## Concept review — November 10, 2026

**A held terminal stands where the owner stood for the lf processes inside
it.** Built. The start receipt of the lf process that takes up the
`LF_TRANSPORT_HELD` handoff writes `processes.held_terminal_id`
(`transport::held_terminal()`, through `record_process_in` on all three
start-row paths). Graph steps receive the same handoff and name it too, as
they sit under an owner. `require_settled_work` seeds its allowed set with
`held_terminal_processes(terminal)`, as it seeds owners.

### Second pass, after the positions/queries/drawing steers — November 11, 2026

Both steers (comments e16443ff, 135b346c) are built and recorded above and in
`docs/dev/server-first.md`. Two parts of "Direction after explore" no longer
match the design that follows from 4a, and are settled here:

- **No snapshot on graceful stop.** Open views resume by position and never
  read a rebuilt screen; only a view opened after a restart does. Ring replay
  plus one nudge is already the path for an incompatible build, so a second
  rebuild path from `screen.ghostsnp` buys one cold-open case at the cost of a
  revision/position match. Dropped; `server-first.md` "Joining late" now says
  bytes plus nudge. Slice 0 (the mode probe) still says how good that cold
  open is.
- **Acceptance "no per-terminal socket".** The holder protocol is one `read`
  stream per connection, kept small on purpose; the registry holds one holder
  connection per terminal and one per view, on one thread. The load the brief
  named is processes and threads. The acceptance line becomes: server thread
  count does not grow with held terminals; `lf ps --json` lists one holder
  LfProcess per build and no owner row per held terminal. The second is
  `held_terminals_list_one_holder_process`; the first is
  `server_threads_do_not_grow_with_held_terminals`.

No open product choice.

## QA — November 11, 2026

`scratch/qa.html`. Fixed in QA: a continuing held view was cleared by the
relay's `handoff()`; the restart test now refuses `ESC[2J`.

### Still wrong, after the QA fixes

Fixed since: a holder is one LfProcess (id from pid + OS start time). The
server that spawns it writes the row and settles it from `wait`; a server that
adopts it at startup writes the row, keeps one idle request connection, and
settles it with no outcome when that closes (the store allows only
succeeded/failed/interrupted, and nobody saw the exit). Both go through
`holder::client::settle`; the holder's idle-out counts running terminals only,
so the watch connection never keeps it. `held_terminals_list_one_holder_process`
covers one row per holder and the adopted holder's row ending after SIGKILL
(it kills rather than idles; the path is the same socket close). A view whose
agent exited while no server ran prints `The agent exited with 3.` and exits 3.
The transport `Fixture` kills every holder its store recorded on drop.
Malformed `lf session attach` args name the expected form.

**A holder unanswered at startup** (QA gap; concept review, third pass,
November 11: "settled, retry", since a dropped holder connection says nothing
about its agents and a CPU-starved holder must not be displaced). Before this
its views got `hosted:false` and the relay exited while the agent ran.

Built (November 11): a holder that keeps its lock and does not answer
`hello`/`list` within 1 s at startup is `Pending` (its directory's terminals
plus the store's running ones for its build); the entrance answers those
`"waiting"`, which the relay retries. Deviation: each pending holder is retried
on its own thread (`Registry::retry`, backoff to 5 s), not in `run`, since a
stopped holder would block the loop for the client's 10 s answer;
`Command::Resolved` lets the loop alone drop it. Gone: terminals settle via
`recorded_exit`, else unknown; `holder::client::settle_departed` completes
unfinished holder rows whose process is dead (also at every startup). Test:
`a_held_view_waits_for_a_holder_stopped_at_server_startup` (SIGSTOP/SIGCONT
continues tick by tick; SIGKILL settles the terminal and the row).

**Gone is a free lock, not a refused socket** (realign, November 11). The
first build read a refused connection as gone. Probe on this Mac
(`kern.ipc.somaxconn` 128): a unix listener that never accepts took 128
connect-and-close attempts and refused the 129th with ECONNREFUSED. Every
retry, server startup and `session.start` adds one, so a holder stopped long
enough would have had its running terminals settled as ended. Now
`Place::claimed` (the holder's kernel lock) decides in `adopt`, `retry` and
`held::Source`; a refusing holder that keeps its lock stays pending. Linux
queue behavior not probed. Test:
`a_holder_that_refuses_connections_and_keeps_its_lock_is_waited_for` (not run
against the old code).

**`session.attach` knows pending holders** (built November 11). The registry
shares its pending holders with the server route: a Session's or a named
terminal that one of them keeps answers `unavailable {resource: "holder",
terminal_id, retry: true}`; `not_found` is left for a terminal no live or
pending holder could hold. The ticket bridge reads the entrance's line and
answers `waiting`/`ended` as those errors in place of `bridged`. Test:
`session_attach_waits_for_a_holder_stopped_at_server_startup` (SIGSTOP: both
request forms retryable; SIGCONT: the same request bridges and reads ticks; a
ticket presented after its terminal ended is `not_found`).

Thread count: `server_threads_do_not_grow_with_held_terminals` starts one
terminal with an open view, then eight more, and allows two threads of slack
for the runtime's pool (`ps -M`, `/proc/<pid>/task` on Linux). It was not run
against a thread-per-terminal build.

Checks, November 11: `transport_tests -- held_ a_held session_attach_waits
server_threads_do_not` 8 passed; `--lib -- transport:: holder::` 87 passed (the
two `snapshot_preview_*` that failed in an earlier full run passed in this one);
`documented_commands::` 4 passed; clippy clean. After compress (match on
held/pending flattened, pending set before its retry threads start): `--lib --
transport::registry::` 10 passed, the same transport filter 8 passed, clippy
clean.

## Concept review with a person — November 11, 2026

Jack Heart ran `/concept-review` while run 1afadabb was on implement.
[concept-review.html](concept-review.html). Proposals, with where they went:

- **Cross-build talk to the server.** Built as the `held.sock` revision.
- Desktop Task shells as held terminals: LOO-830.
- A managed launch gets the server ready itself before failing. Built.
- Plain terminal title reading Reconnecting… while the server is away:
  "not sure"; nothing drawn, as built.
- One place listing the conversations a killed holder took.
- Rollover and resume-takeover have nothing to do for held terminals.

## Concept review, headless — November 11, 2026

Steers e16443ff and 135b346c (positions, queries, view drawing) are built and
recorded above and in `docs/dev/server-first.md`. troubleshooting.md and
sessions.md now agree that a managed launch with the server stopped starts
nothing.

**Next implement direction: a versioned `held.sock` handshake.** Built; see
"Implementation — November 11, 2026" (direction text at
`a18d6a915:scratch/holder.md`). Why: an old relay read a changed answer line as
noise, not an `engine::Error`, so `reconnect_held` retried forever and the pane
froze while the agent ran.

### Second round, against LOO-821/822/823 — November 11, 2026

Jack Heart: "can you talk to 821, 822, 823 and read how their designs have
progressed and the regenerate the concept-review". Read from their scratch
plans and commits (PRs #1879, #1880, #1877); no run was messaged.
[concept-review.html](concept-review.html). Unanswered:

- **Two managed rules.** LOO-821's `WorkSource::is_managed` (hooks, steers,
  server check) and this branch's `transport::managed` (holding) differed on
  a resumed Checkout Session, a Desktop conversation with no Task/Wave, and a
  stopped server. Answered below.
- **Lifetime.** LOO-823 deletes idle ending, `end_detached_after`,
  `DetachedLimit`, `idle_retention` and the native-input ledger. Answered below.
- Status read by the server for every held terminal (LOO-822 makes
  availability come from it). Answered below.

Jack Heart, after asking "what is managed vs eld?": "managed shoudl = held"

One predicate decides both: a launch is held exactly when it is managed
(hooks, steers, server check). LOO-821's `WorkSource::is_managed` (Declared
or Bound) is that predicate, which keeps the earlier LOO-821 answer that a
bare `lf` in a Task checkout gets no hooks; so resuming such a Session is
not held either. Held additionally needs a terminal viewing it (piped and
`-b` executions stay unheld). A managed launch uses LOO-821's
`require_server` (readiness, then fail); `transport::managed`'s own rule and
`ServerMissing` go when the two branches meet. Whichever of LOO-821 and
LOO-824 lands second calls the other's function. Still open: whether a
Desktop conversation with no Task or Wave is managed (today: held here, bare
in LOO-821).

Jack Heart, on `lf resume <session>` of an `lf implement` run typed in a Task
checkout: "bare is correct here". Matches the steer above (not held).

Jack Heart, on "a Desktop conversation with no Task or Wave": "no such
thing? unless you mean soemthing that happens inside a shell in desktop?",
then "i think teh right hting to do there is to hold shells probably".

Read here: `LF_SERVER=1` is set only on Desktop Session panes
(`GhosttyTerminalView.swift:674`), which open Task, Wave and repository
primaries. The repository primary needs no Task or Wave; LOO-823 recorded
"wave/repo should behave exactly like open tasks". Whether LOO-821's
`is_managed` counts a repository primary as Declared/Bound is unchecked.
Desktop Task shells held (an `lf` typed inside one stays bare, a child of
the held shell) is proposal 5a; in this Task or a follow-up is not said.
Then: "if we want to do that. dont need to do it yet". Shells stay on their
owner in LOO-824; holding them is later work, not filed.
Then: "actually we probably do watn to do that for mobile anyway".
Then: "we should amke the desktop hold the pty for all shells in desktop/mobile and throw them away when we throw away task sessions". Filed as LOO-830.

Jack Heart, on a managed launch with the server down (LOO-821 starts it, then fails; LOO-824 fails at once): "821 is correct here". Matches the "managed should = held" steer: require_server, ServerMissing goes.

Jack Heart, on how long a held terminal lives: "823s rule is correct". Held terminals run until their Task is done or abandoned, their Wave or repository conversation closes, or their agent exits. No idle grace for held terminals; sync onto LOO-823 once it lands.

Jack Heart, on the three still-open items: server reads status for every
held terminal: "do it now"; plain terminal title reading Reconnecting…:
"not sure"; LOO-830: "the servers holder should hold the pty".

In this Task, then: the server's hosted execution already parses OSC 7501
Program Status from each held terminal's output (`transport/output.rs`,
`program_status::Records`). For held terminals the server writes that as
the Session's program status (`begin_program_status`, the store path
Desktop's `session.status` writes use) whether or not any view is open.
Desktop's `session.status` stays for what is not held (Task shells, until
LOO-830). The plain-terminal title stays as built (nothing drawn).

Jack Heart, in the LOO-821 conversation (comment 778081f8): "no. in 824 i
explained that there is no such thing other than one you start through a
shell. shells in desktop should be managed by the holder as well, but they
should not be managed (and their pty holding should be more generic than the
lf session one)".

This refines "managed shoudl = held": every managed launch is held, but the
holder also holds things that are not managed. A Desktop shell is held (in
LOO-830, with holding more generic than an lf Session's terminal); an agent
started inside it is bare by `WorkSource::is_managed`. In LOO-824 nothing
changes: lf Session terminals are held exactly when managed; shells stay on
their owner until LOO-830.

## Implementation — November 11, 2026 (handshake, managed rule, status, lifetime)

Built this pass:

- **`held.sock` revision.** `registry::REVISION` (1). `Named` is
  `{terminal,revision}`; the answer `{"answer":…,"revision":N}`. A server
  of another revision answers and admits nothing. `connect` returns
  `OtherRevision`; a plain-terminal view (`attach_with_terminal` first
  connect, `reconnect_held`) calls `transport::replace_view`, which execs
  `os_process::resolve_lf_binary()` as `session attach --terminal T
  [--session S]` with `LF_VIEW_REPLACED=1`; a replacement meeting another
  revision again, or a failed exec, ends with one line. `--moved` and
  `--session-json` are not carried. `Handoff::connect` notes a
  `RecordingGap` for control requests. Desktop's `session.attach` bridge
  treats another revision at its own entrance as an error (same server).
- **Managed rule.** `transport::managed` reads `work_source.is_managed()`
  (`session.rs::WorkSource::is_managed`, identical to LOO-821's, added here
  so the two agree; whichever lands second keeps one). A resumed Checkout
  Session is no longer held. `LF_SERVER` (Desktop pane) and `task_run`
  still hold. Later steer (778081f8): held ≠ managed in general; Desktop
  shells will be held but bare (LOO-830), not built here.
- **Server down.** `start_held` → `open_server`: readiness
  (`lf install server` via the selected installation), then
  `ServerMissing`. Mirrors LOO-821's `require_server`; collapse on merge.
- **Program Status.** The registry hands each held terminal's changed
  `Records` to one `held-status` thread → `record_held_program_status`
  (stream `held:<terminal>`, new stream on a new provider or server).
  `ops::human_session::record_program_status` ignores readings for a held
  Session (`session_is_held`).
- **Lifetime.** Held terminals: `DetachedLimit::Off`, no `idle_retention`;
  only abandonment and completed-Task retirement end them
  (`lifetime.rs`). `a_graph_started_in_a_held_conversation_keeps_it`
  replaced by `a_held_conversation_has_no_idle_ending`. Sync onto LOO-823
  when it lands; the native-input hold is untouched here (823 deletes it).

Tests added: `a_held_view_meeting_another_build_reopens_as_the_installed_lf`,
`a_held_terminal_with_no_view_reports_that_it_waits`.

Checks, November 11: `transport_tests -- held_ a_held session_attach_waits
server_threads_do_not` 10 passed; `--lib -- transport:: holder:: session::`
138 passed; `--lib -- transport::lifetime store::sqlite::program_status
ops::human_session` 55 passed; `--lib -- transport::registry::` 10 passed
after the last edit; `documented_commands::` 4 passed; clippy clean. Left
for gate: Slice 0 mode probe, Swift/Desktop suites, Linux RunPod restart
test, `session_lifecycle_tests`.

Compress (November 11): `Handoff::connect` keeps one `LazyLock` gap;
`Hosted::pass` got its doc back. `transport_tests -- a_held_view_meeting_another_build
a_held_terminal_with_no_view a_held_conversation_has_no_idle` 3 passed; clippy clean.

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

Built (November 11): `repository_session` records `Declared`, as does a
moved Wave or repository primary (`moves.rs`); `managed()` lost its
`LF_SERVER` branch: held = viewed && (task_run || is_managed). Repository
primaries recorded earlier with no WorkSource stay bare;
`prepared_start_and_take_back_…` uses one (Start/Take back are an owner's).
The four held tests that set `LF_SERVER=1` launch `lf resume` (the
repository conversation) instead; `first_native_launch_keeps_only_valid_…`
sets `LF_SERVER=1` to show a bare launch from a pane keeps its owner.

## QA — November 11, 2026 (second pass)

[qa.html](qa.html). Seen working: ten Task Sessions on one holder with one
`lf holder` row in `lf ps`; unviewed Program Status reading `available`; a
`held.sock` revision change (views on the new `lf` reopen, pinned ones end
with one line); a second holder per build; SIGKILL and `launchctl kickstart -k`
restarts with contiguous output; readiness from a stopped server; exits
settled by the next server.

Fixed since: `native_exit::settle` carries the provider's own code (a
held agent exiting 3 records 3 on the launcher Process and the terminal) and
its line reads "claude exited with 3."; an interrupt or a mover still reads
as 0. `NativeExit.resuming` is gone (only the old hint read it). Test:
`a_failed_provider_exit_carries_its_own_code_and_says_so`.

Checks, November 11: focused lib tests 20 passed; `transport_tests` 7
passed; `session_launch_contract` 2 passed; clippy clean. Gate: `session_cli_tests`, `session_lifecycle_tests`,
`work_open_tests` launch the repository conversation and may now need a server.

Jack Heart: "can you stack ontop of 821". Steered the run to lf sync onto jack/loo-821-managed-sessions-hooks-call-the-machine-server (plan: merge_target, 54 unique commits, 82 files).
