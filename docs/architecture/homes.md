# Homes and processes

A Home is one machine's stable Loopflow authority. It owns local processes,
credentials, planning storage, Run records, and OS locks.
Its SSH route may change without changing its identity.

```bash
lf home id
lf wave place product <home-id>
lf ssh <home-id> --wave product wave/operate
```

## Local by default

```bash
lf runs                  # runs recorded on this Home
lf ps --json             # OS-live processes on this Home
lf wave status product        # planning and runtime view resolved here

lf ssh build-home runs   # run the same reader on build-home
lf ssh build-home --wave product wave/operate
```

`lf ssh` is transport, not a second API. The target runs its own `lf`, verifies
its Home identity, resolves its own files and store, and returns the result.
There is no implicit fan-out and no central Run database.

The Home and placement types live in
[`durable.rs`](../../rust/loopflow/src/durable.rs). SSH routing is exposed by
the CLI under [`lf/`](../../rust/loopflow/src/lf/).

## Place Work

`Placement` maps one `WorkRef` to one `HomeId`. It records where Work belongs,
not whether a process exists. `lf wave place` changes placement. It does not launch a process.

## Process topology

```text
shell / automation / Loopflow.app
               |
               lf ---- Linear / GitHub / provider auth
               |
      local store + repository/Git

 Task or Project CLI
          |
          v
 Work Flow position -- exact claim --> boundary Run
                                      |
                                      `--> Home-local Run record
```

The process that directly spawns a child owns that child handle and may cancel
it. Project and Task execution authority comes from the exact Flow-position
claim, not from a deterministic tmux name.

The process that directly spawns a child owns its child handle. Cross-process
recovery requires exact saved process identity and the applicable claim or
lock. A PID, tmux name, parent Run, Work identity or telemetry row alone grants
no signal authority.

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

## Move a Wave without changing its identity

Wave identity is a UUID. The readable Wave locator is `(canonical repository, slug)`.
A bare slug may be ambiguous across repositories and is not mutation authority.

```bash
lf wave relocate <wave-id> --repo <target> --name <slug>
```

Relocation fences the locator, moves authored files, commits the new locator transactionally, and keeps PM, Work, and Home
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
6. Replace the app surface owned by promotion.
7. Persist a switch receipt for recovery or rollback.

The promotion lock lives at the OS account's `$HOME/.lf/promotion.lock` and is held only for the
switch transaction. Ordinary harnesses do not check or hold it. Promotion does
not discover, drain, stop, or settle Runs.

An already-running old process continues with the executable and store path it
selected. On the first published-to-development promotion, it may keep writing
successfully to the prior production store; those writes are then invisible to
commands reading the newly selected clone. A later development promotion may
reuse and migrate the selected development store in place, in which case an
old writer may instead fail against the changed schema. Promotion does not discover arbitrary shells or providers. Retry the
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
- Promotion owns artifact selection and app replacement, not Run
  lifecycle.
- A schema clone protects preview and recovery; it can also leave old writers
  authoring the prior, now-unselected store.

## Next

[Data and persistence →](data.md) maps the stores on each Home.
[Codebase map →](codebase.md) maps CLI and process entrypoints.
