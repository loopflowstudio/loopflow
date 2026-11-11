# Open questions

## LOO-826: waking idle conversations

Steered November 11 (Jack Heart, Task comments 164b4dd8, 50c0310a): "Keep idle
waking: a Task update that lands while a managed conversation is idle should
still wake it, as today, not wait for the person's next turn. Do it from the
server when the update is recorded, with no process per Session and no timer."
"Claude idle waking is open; explore before picking." "Ship the option that
holds up". "Codex waking via turn/start from the server is not in question."

Spikes against installed claude 2.1.322, haiku, scratch dir under /tmp, Machine
load average ~90 at the time:

1. Channels (`--channels server:loopflow` with an HTTP MCP entry): refused at
   startup, "server: entries need --dangerously-load-development-channels".
   With that flag every launch opens "WARNING: Loading development channels …
   1. I am using this for local development 2. Exit". A managed launch cannot
   answer a dialog; not viable. HTTP transport itself was not reached.
2. Async Stop hook blocked on a socket: `/usr/bin/nc -U <socket>` as the hook
   command measured 2.8 MB RSS, one process per idle Session (as `sh -c` too).
   Rejected by the steer's "Don't keep a waiting async Stop hook per Session";
   Claude's `asyncRewake` wake was not exercised.
3. Typing into the terminal through Desktop/Rex: not run. The holder (LOO-824)
   is not on this branch, and typed text lands in the person's prompt draft.
4. Chosen: Claude's cross-session inbox, the socket the old watcher wrote to.
   `~/.claude/sessions/<pid>.json` names `messagingSocketPath`; the auth key is
   `sessions/<pid>.<sha256(socket path)>.key` (matched on a live session). From
   outside, auth + `{"type":"user","priority":"next","from":"loopflow",…}` to
   an idle session: held ("The sender did not attest its permission mode and
   this session bypasses prompts. Review it below, or set
   "crossSessionInbound" to "accept"."). With per-launch
   `--settings '{"crossSessionInbound":"accept"}'`: the idle session ran a
   turn and its transcript had the reply 2.9 s after the write. No resident
   process. The socket directory must be mode 0700 (Claude refuses otherwise).

Choices taken (reversible):

- The hook sends only the inbox key path (`inbox`); the server saves it on
  the AgentProcess (`processes.inbox`), finds the socket in `<pid>.json` and
  reads the key when it sends. No token on lf's wire or in its logs.
- `crossSessionInbound: accept` goes only on managed Claude launches, the
  Sessions the server registers. Any same-user process can message those
  sessions without review. Attesting a permission mode instead was not done:
  lf has none.
- The watcher's only clock is `StoreChanges`' 60 s fallback (missed WAL event,
  checkpoint, sleep). On Linux `StoreChanges` has no vnode watch and polls at
  50 ms, as served subscriptions already do.
- Wake candidates are read from the store, so a restart forgets none. Built
  with the watcher's first revision reading as a baseline (news that landed
  while no server ran waited for the next boundary). Settled by the design in
  concept review 3: the first reading wakes, since claims already skip what a
  conversation heard and the steer asks for waking "as today".
- Claude's `status` in `<pid>.json` is not consulted: the Stop hook can run
  before Claude flips to `idle`, which would skip every turn-end wake, and
  `priority: next` queues a message for a busy session anyway. Idle is
  `session_activity.yielded` alone; `<pid>.json` must still name the thread.
- The Claude inbox key path lives in a new `processes.inbox` column, not
  `endpoint`: Codex and OpenCode readers (`session_connection`, remote client
  checks) treat `endpoint` as an app-server address.
- A wake renders steer sources into the Session's `cwd` (`.lf/prompts/`),
  not the hook's delivery file, which the store does not record.
- Codex wakes only when its last boundary was a Stop; a woken conversation is
  busy until its next Stop. A turn end delivers anything its turn missed.
- An update is recorded `dispatched` before it is sent (after the Claude inbox
  connects). A write that fails after that, or a hook that misses its deadline
  after the server answered, loses that update; nothing is sent twice.
- An unrecorded process exit no longer gets a polling clock; it is reported
  when something else on the Task moves.
- Task comments from tracker sync arrive at the turn-end planning refresh; the
  launch-time `sync_for_conversation` is gone.
- The `task_updates` revision triggers stay (shipped migration); the watcher
  reads them.

Open: bare conversations. The steer says "Deliver at the next boundary only
for bare or unreachable conversations." Under LOO-821's bare rule, bare hooks
ask no server, so a bare conversation gets no Task updates at any boundary
(`docs/sessions.md`: "not registered and gets no Task progress"), matching the
Task brief's "Bare Sessions get none of this". Built as: unreachable managed
conversations get the update at their next boundary; bare ones get nothing.
Giving bare ones next-boundary updates would mean their hooks call the server
again.

Open, from the concept review (November 11):

1. Do steers wake an idle managed conversation? Settled by the design in
   concept review 2 as (a), still reversible: the steer keeps idle waking "as
   today", today a Task comment wakes an idle conversation, and with one
   delivery per comment it arrives as a steer. Built: the wake carries the
   pending steer batch with the Task update. Alternatives were (b) steers wait
   for the next turn, (c) only Task comments wake.
2. Bare conversations (above): get nothing, as built, or next-boundary
   updates, as the steer's words say? The brief's "Bare Sessions get none of
   this" can be read as no registration or push, which fits either answer.
   The conflict is with LOO-821's rule that bare hooks ask no server, and
   with "basically anything that doesnt touch the work model" working without
   one. A middle option: a bare Session whose `work_source` names a Task asks
   the server best-effort at its prompt boundary and logs nothing when none
   answers. Why it matters: whether plain `lf <skill>` in a Task checkout
   hears that Task's progress.
   a. nothing (built). b. next-boundary updates, best-effort server ask.

## Inherited from LOO-821

Decided in the Task conversation, November 10 (Jack Heart's words):

- Steer delivery, lost-until-retried vs arrives-twice: "arrive twice". Build
  the confirmation: the answer names its batch, the hook keeps the printed
  batch beside the delivery file and sends it with its next request; the
  server marks that batch `emitted` and returns other unconfirmed batches older
  than the hook deadline to pending.
- Machines whose server never runs: "we can require remote servers for remote
  work". Managed Sessions require the Machine's server; say so in the gap line
  and in `lf session steers`, naming the server instead of "hook may be
  unavailable or untrusted".

Follow-up, same conversation: "although we should look at what bare lf
--machine does still". What the code does on this branch:

- `lf --machine M` with no command execs a bare `lf` on M over SSH
  (`lf/commands/ssh.rs::run_with_io` → `build_preamble`).
- Every non-preview launch, bare included, gets the turn/tool hooks
  (`prompt/process.rs` `persist` → `ContextDelivery::prepare` →
  `harness/context.rs`), so the remote agent's hooks now need M's server.
- Nothing on that path starts M's server. Only `lf install server` registers
  it; callers are Desktop's terminal connection (`transport/connection.rs`)
  and `server::client::bridge` (managed views). `release/install.sh` and
  `lf machine add` do not; `refresh_after_promotion` restarts one only if its
  socket already exists.
- So a remote Machine used only through bare `lf --machine` records no turns
  and delivers no steers after this change, with one gap line in M's
  `logs/lf.log`. Not run: `mini` has lf 0.15.1 and rejects `--repo-id`.
- Unchecked: on Linux, `systemctl --user` services stop at logout unless
  lingering is enabled.

Clarified: "i guess what i mean is remote work with tasks should require reote
servers". Bare `lf --machine M` stays bare: no server required, and since
the bare rule its two context hooks ask no server, so it logs no gap. Task work on M (`lf
--machine M task run`, `resume --task`) needs M's server, and today nothing on
those paths started it either: views used the server only when `LF_SERVER`
was set (`transport.rs` → `client::bridge`), and Graph runs never asked. So a
Task launch needs its Machine's server before the agent starts, and fails
naming the server if it can't come up (how: the assumption below).

Then: "but it seems like lf --machine X should still be able to work for a
simple skil or graph and basically anything that doesnt touch the work model".
Only managed launches ensure the server. `lf --machine M <skill>` and a typed
Graph run with or without M's server and ask it nothing.

Assumption taken (reversible), where a Task launch starts the server: the
launch opens a connection to the server (hello, welcome) where it reserves its hook capability
(`harness/agent_process.rs`), only for a managed Session
(`WorkSource::is_managed` over `agent_sessions.work_source`). No answer: it runs
`installation::server::readiness_command` (`lf install server` through the
selected installation, as `client::bridge` does), never `ensure` in this
build, then fails naming the server. Fixtures that launch Task or Wave agents
start `lf server run` for their home (`loopflow_test_support::MachineServer`),
so the supervisor is never asked in a test. The probe is the hello the hook
itself needs, not `lf server status`: status reports not ready whenever the
server runs another executable, which would refuse every launch from an
upgraded or branch `lf` beside a working server
(`resumed_session_completes_turns_after_build_replacement_and_viewer_return`
showed it). A Session launched by plain `lf` in a Task's checkout carries
that Task (`work_source` checkout) and is bare: not checked.

- Pi's `pi-event` callback still opens the store each turn. Settled by the
  design: it is an observer and moves with the observer Task
  (`scratch/loo-821-hooks.md`).

Assumptions taken headless with the bare rule, November 11 (reversible):

- Managed is read from how the Work was named, not from where the launch
  came from. `--task` and `--wave` on any command count as a Task or Wave
  command, so `lf --task X implement` is managed.
- Desktop: answered, no mark ("there is no such thing other than one you
  start through a shell", `loo-821-hooks.md`).
- The bare list names launches "typed or run by an agent". Built for headless
  runs only: an interactive bare launch keeps its turn hooks while an owner
  process holds it, because reply state, input acknowledgement and Session
  moves ride on them (`loo-821-hooks.md`, "Not built, and why"). Drop them
  here anyway and let those Sessions read stale, or with the owner in "Bare
  launches go direct"?
  Jack Heart, November 11: "dont really care about intermediate states as
  long as we remove the owner eventually and end up in cleanest place". Kept
  as built; interactive bare hooks go with the owner.
- Sessions recorded `checkout` by an earlier `lf task run` reopen bare. No
  migration rewrites them.

  Direction, November 11: "I prefer idle waking i think?" Steered implement to keep waking,
  driven by the server (Codex `turn/start` over the recorded app-server connection; Claude
  mechanism open, no per-Session waiting hook). Then: "none of those sounds like obviosuly the right answer, so exploring is fine" (options: HTTP MCP channels, a minimal socket-waiting Stop hook, typing into the terminal).
