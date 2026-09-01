# Homes and processes

A Home is one machine's stable Loopflow authority. It owns local processes,
credentials, planning storage, Run records, service managers, and OS locks.
Its SSH route may change without changing its identity.

```bash
lf home id
lf work place wave product <home-id>
lf ssh <home-id> start product
```

## Local by default

```bash
lf runs                  # runs recorded on this Home
lf ps --json             # OS-live processes on this Home
lf status product        # planning and runtime view resolved here

lf ssh build-home runs   # run the same reader on build-home
lf ssh build-home start product
```

`lf ssh` is transport, not a second API. The target runs its own `lf`, verifies
its Home identity, resolves its own files and store, and returns the result.
There is no implicit fan-out and no central Run database.

The Home and placement types live in
[`durable.rs`](../../rust/loopflow/src/durable.rs). SSH routing is exposed by
the CLI under [`lf/`](../../rust/loopflow/src/lf/). The Home daemon lives in
[`lfd/`](../../rust/loopflow/src/lfd/).

## Place Work

`Placement` maps one `WorkRef` to one `HomeId`. Optional Wave goal `owner` and
`home` fields filter automatic startup; they do not replace placement.

```text
origin Home                         target Home
-----------                         -----------
lf work place ... home_B  ------->  Placement(Work, home_B)

lf ssh home_B start product
        |
        `--- SSH transport --------> target `lf start product`
                                      |
                                      v
                                     lfd
                                      |
                                      v
                                Wave listener
```

`lfd` starts only enabled, eligible Work placed on its Home. Placement is a
planning fact. It is not proof that a process exists and never supplies signal
authority.

## Process topology

```text
shell / automation / Loopflow.app
               |
               v
              lf ---------------- Linear / GitHub / provider auth
               |
      local store + repository/Git
               |
               v
              lfd
               |
               v
         Wave listener
          |         |
          |         `-- HTTP, conversation, journal
          v
       resident
          |
          v
 Project controller / Task controller
          |
          v
    provider harness ------> Home-local Run record
```

The process that directly spawns a child owns that child handle and may cancel
it. The Wave listener owns the resident child it spawned. Deterministic tmux
names make resident startup and inspection repeatable; they do not reserve the
Work against independent bound Runs.

None of those local facts becomes generic cross-process Run control. A PID,
tmux name, parent Run, Work identity, or telemetry row cannot prove that a
later process may send a signal.

## Observe processes

```bash
lf ps --json
lf top
lf prune --dry-run
```

The outer command journal records command receipts. `lf ps` and `lf top` join
those receipts to current OS process facts. Completed processes disappear from
the live view. This is observation, not a durable lifecycle model.

`lf prune` removes dead command receipts and may reap only registered orphan
OpenCode process groups whose ownership is known. An unclaimed provider PID is
never killed merely because it resembles a Loopflow child.

Run records intentionally contain no `owner.json`. Durable cross-process
control would require the launcher to create a fresh process scope and publish
PID plus kernel birth identity, boot/Home identity, and the exact process group
or native scope. Every signal would need to revalidate that receipt.

## Run services on the Home

`lfd` serves one Home. It reconciles eligible Wave listeners, receives Linear
and GitHub webhooks, and claims PR landing work. A Wave listener serves its own
conversation, event, playhead, and resident endpoints.

Detached services scrub credentials forwarded from an origin. They use only
authority installed on the target Home. A foreground SSH launch may offer an
explicit account lease for that command.

## Move a Wave without changing its identity

Wave identity is a UUID. The human locator is `(canonical repository, slug)`.
A bare slug may be ambiguous across repositories and is not mutation authority.

```bash
lf work relocate wave <wave-id> --repo <target> --name <slug>
```

Relocation fences the Wave listener and locator, moves authored files and the
journal, commits the new locator transactionally, and keeps PM, Work, and Home
placement joined to the unchanged UUID. A local receipt bridges the filesystem
and SQLite commit boundary so retry can finish verified cleanup after a crash.

## Promote a new artifact

```bash
lf install promote --from-build <path> --preview
lf install promote --from-build <path>
```

Promotion changes the executable selected by future top-level processes:

1. Verify and stage immutable artifacts.
2. Copy the selected planning store.
3. Apply the candidate schema to that isolated copy and prove it can be read.
4. Acquire the machine promotion lock.
5. Atomically select the new artifact.
6. Replace only the known Home services and app surfaces owned by promotion.
7. persist a switch receipt for recovery or rollback.

The promotion lock lives at the OS account's `$HOME/.lf/promotion.lock` and is held only for the
switch transaction. Ordinary harnesses do not check or hold it. Promotion does
not discover, drain, stop, or settle Runs.

An already-running old process continues with the executable and store path it
selected. On the first published-to-development promotion, it may keep writing
successfully to the prior production store; those writes are then invisible to
commands reading the newly selected clone. A later development promotion may
reuse and migrate the selected development store in place, in which case an
old writer may instead fail against the changed schema. Promotion pauses known
Home services but does not discover arbitrary shells or providers. Retry the
operation with a current process after checking which store received the old
write. The isolated clone proves candidate readability, not old-writer
continuity across the selection switch.

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
- Promotion owns artifact selection and known service replacement, not Run
  lifecycle.
- A schema clone protects preview and recovery; it can also leave old writers
  authoring the prior, now-unselected store.

## Next

[Data and persistence →](data.md) maps the stores on each Home.
[Codebase map →](codebase.md) maps the daemon, CLI, and process entrypoints.
