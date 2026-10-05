# Homes and processes

A Home is one machine's stable Loopflow authority. It owns local processes,
credentials, planning storage, command and conversation records, and OS locks.
Its SSH route may change without changing its identity.

```bash
lf id
lf wave place product <home-id>
lf ssh <home-id> --wave product wave/operate
```

## Local by default

```bash
lf mon list                                # Execs recorded on this Home
lf ps --json                           # OS-live processes on this Home
lf wave status product                 # current plan, Task conditions and Session evidence

lf ssh build-home mon list   # run the same reader on build-home
lf ssh build-home --wave product wave/operate
```

`lf ssh` is transport, not a second API. The target runs its own `lf`, verifies
its Home identity, resolves its own files and store, and returns the result.
There is no implicit fan-out and no central execution database.

The Home and placement types live in
[`durable.rs`](../../rust/loopflow/src/durable.rs). SSH routing is exposed by
the CLI under [`lf/`](../../rust/loopflow/src/lf/).

## Place Work

`Placement` maps one `WorkRef` to one `HomeId`. It records where Work belongs,
not whether a process exists. `lf wave place` sets the Home used by Wave schedules
and inherited once by new Projects; new Tasks inherit their Project's Home. It
does not move existing child work or launch a process. `lf wave status` reads
planning, Task conditions, metrics and Session history; no process needs to be running.

## Process topology

```text
shell / automation / Loopflow.app
               |
               lf ---- Linear / GitHub / provider auth
               |
        store + repository
               |
      FlowSession driver --> AgentSession <--> native engine
                             |
                        driving Exec
```

Wave operations are finite attributed conversations. Each Task Flow invocation
runs through the common driver. Cron invokes commands on schedule;
local PR supervision watches and repairs delivery in the invoking process.

The process that directly spawns a child owns its child handle. Cross-process
recovery requires exact saved process identity and the applicable lock. A PID, tmux name, parent Exec, Work identity or telemetry row alone grants
no signal authority.

## Observe processes

```bash
lf ps --json
lf top
lf mon prune --dry-run
```

The outer command journal records command receipts. `lf ps` and `lf top` join
those receipts to current OS process facts. Completed processes disappear from
the live view. This is observation, not a durable lifecycle model.

`lf mon prune` removes dead command receipts and may reap only registered orphan
OpenCode process groups whose ownership is known. An unclaimed provider PID is
never killed merely because it resembles a Loopflow child.

Cross-process control requires exact PID/start identity and the applicable
conversation/provider generation. Revalidate native scope or exclusive process
group before signaling. A driver may disappear while its engine survives;
recorded endpoints alone do not prove liveness.

## Independent bridges

`lf discord serve <wave>` is a foreground bridge from a configured channel to
bounded conversations. It has no Wave cursor or inbox authority. Cron and Task execution
do not require a daemon or bridge. See [Discord](../waves.md#discord-bridge).

## Move a Wave without changing its identity

Wave identity is a UUID. The readable Wave locator is `(canonical repository, slug)`.
A bare slug may be ambiguous across repositories and is not mutation authority.

```bash
lf relocate <wave-id> --repo <target> --name <slug>
```

Relocation fences the locator, moves authored files, commits the new locator transactionally, and keeps PM, Work, and Home
placement joined to the unchanged UUID. A local receipt bridges the filesystem
and SQLite commit boundary so retry can finish verified cleanup after a crash.

## One main Home

```bash
lf install                            # update the installed release and main Home
uv run python scripts/install.py local # build an experimental CLI
LF_HOME="$(mktemp -d)" local-bin/lf wave list --json
```

All ordinary commands use the installed CLI and `~/.lf`. Source CLI commands
forward there before opening a store. Task Flows, Flow steps, sessions and
agent tools inherit the same Home; no source-specific or installed-development
Home exists.

`LF_HOME` explicitly selects an empty, disposable experiment. Its database is
always `$LF_HOME/loopflow.db`; no other variable selects a store. Every variable
Loopflow sets or reads is listed in [Environment](environment.md). Its schema is
initialized once and must match on subsequent opens. A schema mismatch requires
a new experiment. Loopflow neither copies main data into it nor upgrades,
repairs, restores or promotes its contents.

Published installation verifies immutable artifacts, validates the candidate on
a temporary database snapshot, and advances only the main database under the
machine promotion lock. The snapshot is validation input, never a second live
Home. Published switch receipts support interrupted release installation;
they do not choose ordinary command data directories.

Artifact switching lives in
[`machine_install.rs`](../../rust/loopflow/src/machine_install.rs) and the
install command implementation under [`lf/commands/`](../../rust/loopflow/src/lf/commands/).

## Boundary contracts

- Home identity is stable; network route is replaceable.
- Commands and read surfaces act locally unless explicitly routed with
  `lf ssh`.
- Placement selects where Work belongs, not whether it is currently running.
- Detached processes use credentials installed on their Home.
- Direct child handles are local capability; inferred process ownership is not.
- Promotion owns artifact selection and app replacement, not conversation
  lifecycle.
- Published preview uses a temporary snapshot; all ordinary writers share the main Home.

## Next

[Data and persistence →](data.md) maps the stores on each Home.
[Codebase map →](codebase.md) maps CLI and process entrypoints.
