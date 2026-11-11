# LOO-819 implementation assumptions

Explore is open; no vision has been selected. See `explore.html` and the
Explore section of `universal-server.md` for questions 1 and 2. The kickoff's
following choices are proposals:

- Keep the existing `lf server run` installation and LF-owned PTYs. Native Rex
  launch remains an investigation; no program/icon change is promised.
- Start with four projection workers, 256 queued keys and 128 client slots;
  measure them against the stated freshness and whole-process CPU targets.
- Keep thirty-second filesystem reconciliation and three-second degraded
  watching until measurements justify another value.
- Treat the CPU/latency numbers in `universal-server.md` as proposed completion
  targets, not measured improvements or quoted product requirements.

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
servers". Bare `lf --machine M` stays bare: no server required, its gap
closes when "Bare launches drop their hooks" lands. Task work on M (`lf
--machine M task run`, `resume --task`) needs M's server, and today nothing on
those paths starts it either: views use the server only when `LF_SERVER` is
set (`transport.rs` → `client::bridge`), and Graph runs never ask. So a Task
launch ensures its Machine's server (`installation::server::ensure`) before
the agent starts, and fails naming the server if it can't come up.

Then: "but it seems like lf --machine X should still be able to work for a
simple skil or graph and basically anything that doesnt touch the work model".
Only Task launches ensure the server. `lf --machine M <skill>` and a Graph with
no Task run with or without M's server; their hooks fail at connect (no
socket, no two-second wait) and record one gap line.

Not built, and why (`loo-821-hooks.md`, Open): `installation::server::ensure`
registers a supervised launchd or systemd job for the selected home, and the
integration tests launch Task work under temporary homes with no server. Which
is wanted: Task-launch fixtures each start and stop `lf server run`; the launch
only checks that a server answers and names `lf install server`; or something
else? Until answered, a Task launch with no server runs as a Task-less one
does: one gap line, steers wait.

- Pi's `pi-event` callback still opens the store each turn. Move it behind a
  server operation in this Task, or leave it for the Task that turns observers
  into server registrations? Left as is here.
