# Slice 2: delete Radio and channel identity

## Implement

- Delete the Radio CLI, bus store/schema/runtime/listener/retention, and the
  channel-family identity model.
- Delete `LF_CHANNEL`, dotted channel placement helpers, `MessageOp::Say`, and
  machine-authored bylines on chat/message DTOs.
- Add the next migration dropping agent-bus tables.
- Remove hidden compatibility commands and the legacy channel-open event.
- Replace project-promotion and builtin uses with typed evidence already owned
  by Work/Project/Wave.
- Update Rust, Swift, fixtures, help, docs, prompts, and command-resolution
  matrices with no alias or dual reader.

## Preserve

- Human Wave conversation and generic Work Steer/Turn behavior.
- `lf chat` as the current human surface pending the server-topology design.
- Durable parent/child Work relationships; do not replace Radio with another
  mailbox.

## Done when

- [ ] Radio/channel modules, commands, exports, store APIs, and current tables
      are absent.
- [ ] `LF_CHANNEL`, channel roles/positions, `MessageOp::Say`, and machine
      bylines are absent.
- [ ] Project promotion and builtin prompts use typed evidence, not Radio.
- [ ] Exact current-source search is zero outside historical migrations.
- [ ] Focused CLI/migration/Wave/Swift proofs, fmt, and clippy pass.
