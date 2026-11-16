# LOO-926: route typed operations, not command lines

## Problem

`Operation = Server | Read | Write` (`rust/loopflow/src/server/wire.rs`) is the
one request type, and `Read::read` / `Write::run` are the one implementation of
each operation. But only the server's socket carries the value. Every hop
between `lf` processes carries argv instead, so routing is written once per hop
and each hop re-parses with Clap. The Task brief lists the duplicates. Mapping
the code at `470da475a1` showed it is worse than the brief:

- **Three SSH launch sequences, not two.**
  - `lf/commands/ssh.rs::run_with_io` (:71–386).
  - `machine/task.rs::run_remote` (:238–329, engine Task writes).
  - `Write::prepare_session` (`write.rs:1113`), which goes through
    `work_route::Forward::run` into `ssh::run_with_io` with argv from
    `Write::cli()`.
  - "the destination said" appears in `run_remote` and `prepare_session`.
- **The CLI never calls `Write::run`.** Its only caller is the server
  (`server/service.rs:1317`). CLI writes go through `mutation::run`
  (`mutation.rs:51`, gate `accepts` :9) or `dispatch` arms. They reach the
  same `ops::*` / `engine::*` leaves, except where they diverge:
  - config set/unset: the CLI calls `setting::declare`, served calls
    `engine::config::set`;
  - session writes: the CLI calls `session::mutate`, served calls
    `session::renamed/bound/moved/status_recorded`.
- **Forwarding is decided in four places, not three.**
  - `--machine` in `bin/lf.rs:1703`.
  - `work_route::forward` (`work_route.rs:258`).
  - `engine::task::remote/remote_on` (`loopflow-execution/src/engine/task.rs:308,321`).
  - `transport::delegate_current` (`transport.rs:247`), which sends the
    local server `ServerOperation::SessionStart { argv: std::env::args() }`.
- **The engine cannot send an `Operation`.** `Read`/`Write` live in the
  application crate, `engine::task` lives in `loopflow-execution`. So the
  engine spells argv instead: `spell_run` :347, `spell_task` :368. The same
  spelling feeds the detached local launch `engine::launch::start`
  (`launch.rs:142`) for served `task.run`/`task.ship`, and `graph.rs:19`,
  `skill.rs:16`, `pr.rs:212`, `repo.rs:29`, `checkout.rs:238`,
  `creation.rs:156`.
- **Parses per invocation.** `bin/lf.rs` parses up to five times on the main
  path (:1548 config fast path, :1689 `--open`, `navigation::machine_invocation`,
  `ssh::parse_remote_command`, :1752), plus `preview_invocation` (:2114). About
  23 commands are intercepted before store setup. Config get/set runs at
  :1571–1632 and again at :2863–2903, the second only for
  `--machine <this machine> config …`. The `unreachable!` is at :3051–3056.
- **`Read::argv` (`read.rs:387`) forwards nothing.** Its callers:
  - the "has no --subscribe" error (:988);
  - `session_filter` (:1014), which re-parses argv to borrow Clap's defaults;
  - `tests/served_reads.rs:751,1254,1273`, which compare served output with
    CLI output.
- **Of the 23 served-flag call sites, only 6 drop env selection**
  (group A below). The rest stay:
  - Process/journal identity (`journal/mod.rs:366,421,467,606,1185`): a
    request is a trace, not an OS Process.
  - The read cache clock (`observe::active` ×3).
  - No terminal (`provider_account::operations::served`, used by
    `narrate!`/`diagnose!`, the Chrome-profile prompt and
    `manual_authorization`).
- **Swift.**
  - `RegistryQuery.swift` already sends typed operations for almost everything
    (68 `serve`/`write` calls). `ServedRead`/`ServedWrite` keep an `argv`
    beside each `encode` for the no-server fallback (:826–840).
  - `onMachine` (:911) drops `serve`/`write`, so every remote-Machine call
    becomes `lf --machine M --repo-id R <argv>`.
  - There is no codegen. `lf server describe` emits JSON Schema
    (`server/schema.rs`), and `ServerProtocolTests.swift` only checks field
    sets against it.

## Demo

Machine `mini` is added, and Task LOO-123 executes there.

```bash
lf task hold LOO-123 --json            # answer comes from mini
lf --machine mini session history list # mini shows one `lf server stdio` Process: task.hold
```

With a stub `ssh` that records its input, the remote command is
`exec 'lf' 'server' 'stdio' …`. Stdin is two JSON lines, `hello` and
`{"type":"request","operation":{"kind":"task.hold",…}}`, and argv carries no
Task words.

Against a destination built before this change, the call fails and names the
destination's version and the update command. It does not exit 0 silently.

## Chosen approach

Docker's shape: the CLI and the daemon share one API, and remote is a
different address, not a different code path. Tailscale's: the CLI is a thin
client of one typed request.

### One typed entry: `lf server stdio`

- **Protocol.** `lf server stdio` speaks the socket protocol on stdin/stdout
  for one connection. The client sends `hello`; the entry answers `welcome`,
  or `error incompatible` on a revision mismatch (the same check as
  `service.rs:392–408`). Each `request` gets one `response` or `error`.
- **Refused here.** `subscribe` and every `ServerOperation` (session.start,
  attach, mobile.*, upload, boundary) get `error invalid` naming the resident
  server. No new `ErrorCode`, so PR 1 changes no wire type.
- **Nothing resident.** It is an ordinary `lf` Process on the destination,
  admitted and recorded, so the destination's history shows the call. It is not
  the shared server, so it doesn't run under `request::isolate` / `served`.
- **Why visible, not `__op`.** `bin/lf.rs:1756,1780` exit 0 for any `__*`
  they can't parse. An older destination would turn `lf __op` into silent
  success with empty output. `lf server stdio` on an older `lf` is a Clap
  error (exit 2). The sender turns exit 2, or a stdout with no `welcome`, into
  "mini runs lf <version>; update with <cmd>", using the version that
  `connection::probe` already reads.
- **Identity.** `LF_EXPECTED_MACHINE_ID` stays in the preamble, checked by
  `validate_expected_machine_process` before admission. `machines.md:47–52`
  requires every receiving invocation to check it, and env is safe here:
  there is one caller per process.

### One executor, one route (as built in PR 1)

No `dispatch` module yet. `Write::run` routes and `Write::run_here` runs here
(`write.rs:64`); the server calls `run`, `lf server stdio` calls `run_here` or
`Read::read`. PR 2's CLI dispatcher is where a shared `execute` earns a name.

- **Route.** `Write::remote` (`write.rs:1144`) covers
  task.run/ship/hold/claim/unassign, reading `remote_task` and
  `remote_ownership_writer`, and session.resume, through `work_route::forward`,
  whose `Forward` carries the Wave and repository it resolved. It returns one
  `Remote` (resolved `Machine`, repository, the write naming exact Work);
  `execute` asks it only when routed, and `prepare_session` adds only
  `move_session` before `Remote::call` and `prepared_remotely` after.
  `session.new` never forwards, so it never routes.
- **Engine.** `engine::task::{run, ship, hold, ownership}` are local-only;
  `remote`, `remote_on`, `RemoteTaskOperation` are deleted.
- **No re-forwarding.** An operation that arrived on `lf server stdio` runs
  here, never re-routed.

### One SSH launch

`machine::ssh::call(target, repo, messages) -> Vec<ServerMessage>` in
`loopflow-execution`, generic over JSON:

- `resolve_target`, no up-front probe: the destination checks
  `LF_EXPECTED_MACHINE_ID`, and the version is read only after exit 2;
- `forwarding_environment`;
- `build_preamble(["lf", "--repo-id", id, "server", "stdio"])`;
- `run_ssh_io` with the messages on stdin;
- stderr lines become `journal::request::diagnostic`, and exit 2 becomes the
  skew error.

`run_remote`, `Write::prepare_session`'s SSH half and the "destination said"
copies fold into it. `prepare_launch` stays in `write::Remote::call` (run,
ship) and the CLI's `ssh::run`; LOO-913 (#1953) deletes both, see Decisions.

### Directory on the destination

`Read`/`Write` variants carry `directory: PathBuf`, the sender's path. The
entry takes `--repo-id` as today, resolves it with
`work_route::repository_path`, and runs `operation.at(path)`, one method that
replaces the `directory` field. That method stays until PR 2 moves selection
to the request (see Slices). The Task source (`LF_TASK_SOURCE`, prepared by
`resolve_task_source`) stays in the preamble in PR 1. In PR 2 it becomes a
field of the request.

## Alternatives considered

- **`lf __op` hidden entry.** Blocked: silent exit 0 on an older destination
  (above).
- **Host the full server over stdio** (proxy to `service.rs` connection
  handling). This brings subscriptions, limits and tickets to a process that
  answers one request. Deferred: nothing needs a remote subscription yet.
- **Move `Read`/`Write` into `loopflow-execution`** so the engine can send
  them. That is the largest diff, and it makes execution depend on every
  application handler. Moving the route decision up to the caller removes the
  engine's need to forward at all.
- **Selection in `Hello`** (expected Machine, repository). Hello is per
  connection; a socket connection's requests span repositories. Selection
  belongs on `Request` (PR 2).

## Slices

The Task brief says "Large enough to stack: the typed remote entry first, then
the Rust argv tables, then Swift." Each PR below is its own Task. The later
two are filed when this one publishes.

### PR 1, this Task: typed remote entry and one route

Deletions first:

1. Delete `engine::task::{remote, remote_on, RemoteTaskOperation}`,
   `machine::task::run_remote`, `Write::cli()`, the SSH half of
   `Write::prepare_session`, the served branches of `work_route::forward`
   (:285,373,436), and the test `routed_writes_spell_the_command_ssh_runs`
   (`write.rs:1428`).
2. Add `machine::ssh::call` and `lf server stdio` (`ServerCommand::Stdio`;
   help in Clap, `docs/dev/machines.md` and `docs/dev/server.md` updated).
3. `Write::run` routes (`Write::remote`, `prepare_session`) and
   `Write::run_here` executes; `Operation::at` rewrites `directory` on the
   stdio entry.
4. Tests (`tests/machine_commands.rs`):
   - `served_ownership_routes_to_its_writer_and_never_falls_back_locally`
     (:3968) passes unchanged.
   - New: a recording `ssh` stub shows argv `lf --repo-id … server stdio` and
     stdin `hello` + `request{kind:"task.claim"}`.
   - New: a `remote()` stub answering exit 2 to `server stdio` yields the skew
     error with the update command.
   - New: served `task.run` for a Task routed to a `preview_machines()` peer
     returns the peer's run (no test covers remote run/ship today).
   - Update the `remote()` argv-line assertions that covered engine-routed
     operations.

Still argv in PR 1:

- `lf --machine` and `work_route::forward` from the CLI (`mutation.rs`,
  `dispatch` arms);
- detached `launch::start`;
- `transport::delegate_current`;
- Swift `onMachine`;
- `machine::ssh::request` (Session move, Task location, workspace arrival).

Each one goes in a named PR below.

### PR 2: Clap produces an `Invocation`; one dispatcher

- `Cli` → `Invocation { selection: Selection { directory, repository, wave, task, machine }, action }`
  in one parse. `action` is an `Operation`, or a terminal-local command
  (install, holder, hooks, desktop shell, attach). `Selection` moves to
  `ClientMessage::Request` as a required nullable field. Every DTO fixture
  changes, along with Swift encoding. The per-variant `directory` fields and
  `Operation::at` are deleted.
- The CLI calls `dispatch::route` + `execute`, then renders the typed result.
  Deleted:
  - `mutation.rs` (all of it);
  - CLI `work_route::forward`/`Forward`;
  - `ssh::run_with_io`'s argv rewriting (`resident_args`, `parse_remote_command`,
    `runs_here`, `navigation::machine_invocation`);
  - config dispatch at `bin/lf.rs:2863` and its `unreachable!`;
  - the extra parses;
  - `Read::argv` and `session_filter`'s re-parse (Clap's defaults become
    `SessionListArgs → Read` construction).
  - Group A served guards: `work/wave/context.rs:21,31,280`, `ops/run.rs:320`,
    `ops/task/remote.rs:69`. Resolvers take the selection, and env is read once
    at the Clap boundary.
  - `EnvGuard` for `WAVE_ID_ENV` and `WORK_DECLARATION_ENV`, and `CwdGuard`
    except where a child agent inherits cwd.
  - The CLI and served config/session writes converge on one function each.
- `--machine` sends the `Invocation`'s operation through `machine::ssh::call`.
  A command with no `Operation` fails, naming `ssh <machine> lf …`
  (Decisions).
- The detached launch passes the typed operation on the child's stdin
  (`lf server stdio`, carried with `LF_LAUNCH_REPORT`). Deleted: `spell_run`,
  `spell_task` and the `launch::start(directory, &[String])` argv form.
- `session send`/`receive`, `task location` and workspace arrival get
  Read/Write kinds and go through `machine::ssh::call`; `ssh::request` is
  deleted.
- `aliases.rs` is deleted (52 moved spellings, "delete after 0.15").
  `transport::delegate_current` sends the operation, not `std::env::args()`.
- Snapshot-test the Clap → `Invocation` table, one line per command, in the
  manner of Jujutsu's CLI snapshot tests.
- Route once, in `Write::run`: `remote()` → `Remote::call::<$output>` →
  `WriteResult`, else `run_here`. The six `match remote { Some … None … }`
  arms in `writes!` become local-only, and the CLI's hand-routed copies
  (`mutation.rs` hold and claim/unassign, `write::run_task`) become calls to
  the dispatcher. session.resume's `move_session` before and
  `prepared_remotely` after move into that one routed path. `ContinueRun`
  stays while execution cannot name `Write`; every caller passes `run_task`.

### PR 3: Swift

- `onMachine` keeps `serve`/`write` and sets `Request.selection.machine`; the
  local server routes it through `dispatch::route`.
- Delete every `argv` in `ServedRead`/`ServedWrite` and the no-server argv
  fallback (`RegistryQuery.swift:826–840`). The Mac app talks to the server
  it bundles.
- Replace the direct argv calls (`config repo`, `task location`,
  `session resume --explain`, `task watch-files`, `taskGet --subscribe`) with
  their reads or subscriptions. `watch-files` stays a process if it has no
  operation.

## Decisions (reversible; recorded in `scratch/questions.md`)

- **LOO-913 order.** #1953 is open, and its branch has not synced past
  `606c13c2d7` (before the crate split in #1952 and #1954). PR 1 builds on
  `main`; the one remaining `prepare_launch` call sits in `machine::ssh::call`.
  If #1953 lands first, rebase onto it and drop that call. Before merging
  either one, check with `merge-base --stat` for the stale-parent revert.
- **`--machine` without an `Operation` (PR 2)** fails, naming the ssh spelling.
  Documented remote commands checked against the Read/Write kinds:
  - covered: `task list`, `session history list`, `skill run`, `graph run`,
    `wave-operate` (skill);
  - not covered: `lf --machine X install server` (`docs/troubleshooting.md:166`),
    which becomes `ssh X lf install server`. PR 2 lists the rest from its
    snapshot table.

## PR 1 as built (2026-11-16)

Departures not covered above:

- **CLI `lf task hold <id>`** routes typed: the engine used to route it, and
  CLI `work_route` never did.
- A served `task.run` to a peer starts detached there and returns at
  registration; the argv path ran `lf -b task run` in the SSH session.
- `Write::cli` became `Write::session_spelling`: local Session preparation
  still reads Clap's arguments until PR 2. `ssh::run_with_io` keeps its IO
  for `mutation::run`'s CLI forward.
- Two CLI paths had routed only through the deleted engine `remote_on`, and
  now ask `Write::remote`: `lf task claim/unassign` for a Task whose move from
  here was sent (mini answers whether it arrived), and the Graph a move
  continues on arrival. `ops::session_move::{move_task, move_session}` take
  the application's `write::run_task` as a `ContinueRun` function pointer,
  since execution cannot name `Write`.

## Remaining for PR 1

Nothing known; gate next.

A fourth lf-to-lf channel the inventory missed: `machine::ssh::request`
(`loopflow-execution/src/machine/ssh.rs:45`) launches `lf <argv>` with its own
preamble beside `ssh::call`. Callers: `session_move.rs` (`session send`,
`session receive` with a stdin body, `--planning-key`), `task_location.rs`
(`task location --request --json`), `open/workspace.rs:636` (`--arrive-task`,
`--into`). None has an `Operation` kind, so typing them changes the wire:
PR 2, listed there.

## Done when

- A Task write whose Task runs on another Machine (run, ship, hold, claim,
  unassign) reaches it as typed JSON. So does a session.resume that
  belongs to another Machine (`session.new` never forwards).
- An older destination fails with its version and the update command.
- `rg 'fn run_remote|fn remote_on|RemoteTaskOperation|\.cli\(\)' rust/*/src`
  finds nothing (`fn cli(` also matches unrelated cmux and Rex helpers).
- (PR 2/3) `rg 'ssh::request' rust` finds nothing, and
  `rg 'fn argv|mutation::|aliases::' rust/loopflow/src` and
  `rg 'argv' swift/Loopflow/Services/RegistryQuery.swift` find nothing.

## Docs (PR 1, written)

- `docs/dev/machines.md`: routing table row, and the typed write, its
  example and the skew error after it.
- `docs/dev/server.md`: boundary diagram line, `lf server stdio` under Commands.
- `rust/loopflow/src/server/README.md`: the stdio carrier, after the remote
  carrier paragraph.

The skew message in the docs reads ``mini runs lf 0.15.2, which cannot take
this operation; update it with `ssh -- mini 'lf install'` ``. Implement
matches it, or changes the docs with it.

## Checks

- `cargo test -p loopflow --test integration machine_commands::`: 58 pass, including `a_served_run_for_a_task_mini_runs_starts_it_there` (stub mini stdio) and the move continuation's real Graph on mini (implement).
- `cargo test -p loopflow --lib server::stdio:: lf::commands::write::tests:: adapter_tests:: lf::tests::`; `-p loopflow-execution --lib engine::task:: machine::ssh::`: pass (implement).
- `documented_commands::`, `cli_discovery::`, `served_reads server_operations_match_their_lf_commands`: pass (implement).
- `cargo nextest run -p loopflow -p loopflow-execution --lib -E 'test(lf::commands::write::tests::) | test(adapter_tests::) | test(session_move::)'`: 15 pass (implement).
- `cargo clippy --all-targets -- -D warnings`: clean (implement); gate reruns.
- Loopback sshd (`tests/transport/ssh_shells.rs`): CI.
