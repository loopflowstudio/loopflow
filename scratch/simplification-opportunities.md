# Simplification Opportunities

Area: LOO-824's diff against LOO-821 (held terminals, the registry, the
holder, their store rows). About 9,000 lines added and 1,300 deleted.

## Product intent

A managed conversation, whether Task, Wave or repository, runs in a PTY that
outlives every lf process. An install or a server restart pauses its views and
they continue. lf records the conversation beside it and is never what keeps
it alive.

## Opportunity 1: take LOO-823's lifetime rule (done)

Built in the LOO-823 merge; see `holder.md` "Stack merges".

## Opportunity 2: one kind of execution owner

**Misalignment**: A conversation's agent runs under either an owner process
(`lf session serve`) or a held terminal, and every layer carries both:

| Layer | Two-way split |
|---|---|
| Execution | `hosted::Child::{Local, Held}` |
| Host identity | `Host::{Process, Terminal}` |
| Store owner | `store::sqlite::Transport::{Owner, Terminal}`, with `session_column()` and `table()` |
| Sessions and retirement queries | `COALESCE`/`EXISTS` pairs over `processes` and `held_terminals` (`sessions.rs:280`, `:293`, `:664`, `:764`; `task_retirement.rs:100`) |
| Attach | `bridged`, carrying `owner_lf_process_id` or `terminal_id` with the other null |

`held_terminals` repeats the owner's viewing columns (`viewer_hold`,
`viewer_hold_at`, `viewed`) so the shared queries can read either table.

**Symptom**: Each new question about a conversation's liveness is asked
twice, once per table. "Detached", "retains the workspace" and "can be
taken over" mean different code for the two owners.

**Realignment**: Every interactive conversation's agent is a held terminal.
Bare launches run direct (LOO-820). Desktop shells are held but not managed,
and the owner process goes (LOO-830). `Child`, `Host` and `Transport` then
have one arm, and the store queries read one table. That is sequenced work,
not a change for this branch. Inside LOO-824 the rule is to keep the split
in those three enums: new code takes a `TerminalId` or a `Host` and never
matches on both owners.

**Cascade**: Once the owner is gone, these go with it:
- the per-owner viewer socket and its reconnect spool;
- `lf process rollover` and the takeover judgment (README "Nothing else ends
  an owner…" through the rollover paragraph);
- the `.ghostsnp` exit snapshot, which the ring replay replaces;
- the `owner_lf_process_id` half of `bridged`;
- `transport_lf_process_id` on `agent_sessions`.

## Opportunity 3: managed is decided once, by the command

**Misalignment**: LOO-821 decides managed by the command
(`Cli::names_work`). This branch also decides it by the recorded row
(`work_source ∈ {declared, bound}`) in two places: `transport::held` and
`agent_process::require_server_for`. That second rule exists because Desktop
reopens a pane by Session id (`session resume --start-if-free session_…`),
which names no Work.

**Symptom**:
- `WorkSource::is_managed` survives only for this read.
- The launch path opens the store to decide whether to hold. When the store
  can't be read, the launch is unheld.

**Realignment**: The reopen names its Work. Desktop already knows a pane's
Task, and the repository conversation is reached by `--repo`. With Desktop
sending `--task`/`--wave` on every reopen, the row read and `is_managed` are
deleted and held means `managed`. A typed `lf resume <id>` of a Task
conversation then runs bare, which matches "the cli comamnd used should
direct down different codepaths". This is question 1 on
`concept-review.html`. Answered 1a, "double coverage for now" (see
`questions.md`): the row read stays until a later cut.

**Cascade**: `held(viewed, managed, session)` becomes `viewed && managed`,
and `require_server_for` becomes `require_server` behind `managed`.

## Not cut: `Hosted::pass`

`transport/hosted.rs` `pass` is ~600 lines, the owner's loop moved whole;
the per-viewer frame handling (its first ~250 lines) is the natural first
extraction. Left alone while both owners drive it; reshape when
opportunity 2 removes `Child::Local`.

## Aligned areas

- **The terminal's directory as the receipt.** `meta.json`,
  `exit-record.json`, `native-client.json` and the holder's `exit.json` sit
  under one directory. That directory is deleted only after the store has
  the exit. It works when the store, the server or the holder is gone, which
  is the recording-gap rule applied to exits. Moving any of it into holder
  memory would lose the holder-gone case.
- **The `Hosted` extraction.** The owner's per-execution logic moved into one
  type that both owners drive, instead of being copied into the registry.
  When `Local` goes, it deletes cleanly.
- **The holder's protocol.** It has eight verbs and knows nothing about
  Sessions. That is the right size for the one process that has to outlive
  every build.
- **`held_terminals` as its own table.** A terminal is not a Process, and
  after LOO-830 a held shell has no Session, so neither `processes` nor
  `agent_sessions` could hold it.
