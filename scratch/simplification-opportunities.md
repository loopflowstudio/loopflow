# Simplification Opportunities

Question: what code can LOO-824 delete?

## Product intent

A managed conversation (Task, Wave or repository) runs in a PTY the holder
keeps, and it outlives every lf process. lf records the conversation beside
it and is never what keeps it alive.

## Already deleted, or no longer reached

- LOO-823's idle ending: `end_detached_after`, `DetachedLimit`, idle retention
  and the native-input ledger. They are gone on this branch, and held
  terminals take the shared rule (`holder.md` "Stack merges").
- `ServerMissing` and `open_server`, replaced by LOO-821's `require_server`.
- Background Task-conversation starts (`primary.rs::start_in_background`) and
  restores (`restore.rs`) now go through `start_held`. No managed
  conversation starts on an owner any more.

## Opportunity 1: the owner serves only bare conversations, so delete it with LOO-820

**Misalignment**: After this change, `lf session serve` owns only bare
interactive launches: `lf -i`, and `lf implement` typed at a terminal.
Everything the owner does to keep a conversation alive across terminal loss
and installs now exists twice. The holder does it for managed conversations.
The owner does it for bare ones, which per LOO-820 should run directly in the
person's terminal with no lf in between.

**Symptom**: These all exist only for owners. A rough count is about 3,000
lines, plus the arms that read the owner in shared code:

- `lf process rollover` and the resume takeover judgment
  (`lf/commands/rollover.rs`, 885 lines). They exist for owners left behind
  by an install.
- The owner viewer socket's reconnect spool and grace (`transport/stream.rs`,
  and the `resume`/`reconnect` messages).
- The `.ghostsnp` exit snapshot (`transport/snapshot.rs`) and `lf resume`
  painting it.
- `Child::Local`, `Host::Owner` and `store::sqlite::Transport::Owner`, with
  the `processes` arm of each paired Session and retirement query
  (`sessions.rs:280`, `:293`, `:664`, `:764`; `task_retirement.rs:100`).
- The Desktop owner bridge: `LF_SERVER` (`transport.rs:1977`), the
  `owner_lf_process_id` half of `session.attach` and `bridged`, and owner
  `Tickets`. Today it is reached only when Desktop shows a bare conversation
  that is still running in someone's terminal.

**Realignment**: LOO-820's direct bare launch takes the owner's last
conversation, so the owner process goes entirely rather than staying as an
owner with fewer features.

**Cascade**:
- `transport/README.md` shrinks to its held-terminal sections.
- `Hosted` loses its `Local` branches.
- `lf session list` no longer shows `detached` for bare runs.

## Opportunity 2: hold Desktop shells, then delete the shell runtime (LOO-830)

**Misalignment**: Desktop Task shells keep their own runtime
(`desktop::shells`: execution receipts, reconnect, saved screens). That makes
a third owner of PTYs, beside the owner process and the holder.

**Realignment**: Shells become held terminals with no Session. They are held
but not managed, as already decided.

**Cascade**:
- Desktop's `session.status` path for Program Status goes. The server
  already reads OSC 7501 for every held terminal.
- Shell viewing moves from `processes.viewer_hold` to `held_terminals`.

## Not deletable yet

The row read for a conversation reopened by id (`transport::held`,
`require_server_for`, `WorkSource::is_managed`) stays, as "double coverage
for now" (Jack Heart). Desktop panes name their Work; a Session link does not.

## Aligned areas

- The terminal's directory as the exit receipt. It works with the store,
  server or holder gone.
- `Hosted`, the per-execution logic shared rather than copied. When `Local`
  goes, it deletes cleanly.
- The holder's eight-verb protocol, which knows nothing about Sessions.
