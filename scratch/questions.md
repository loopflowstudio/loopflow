# LOO-821

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

- The load guidance in `AGENTS.md` ("**Load:** one Machine runs hundreds of
  concurrent agents…") and `builtins/task/skill/implement.md` ("Write for the
  load it will carry") appeared uncommitted in this checkout from another
  Session during publish; `lf sync` checkpointed it into this branch. Kept in
  PR #1879; move it to its own PR if it shouldn't land with LOO-821.
