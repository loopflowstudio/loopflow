# Open questions

## Sync onto origin/main 45a0eecc94 (#1887, held terminals): aborted 2026-11-12

Both sides redesigned the server protocol independently. Choices needed before the merge:

- **Revision.** Both moved `REVISION`/`MIN_REVISION` 4 → 5 with different contents. Should the merge be revision 6? Main's 5 may already be in a release; this branch's 5 also drops `MAX_MESSAGE` to 1 MiB.
- **`session.start` collision.** The branch's Write `session.start {id}` (`lf session resume --start-only`, mobile continue through the shared owner) and main's `ServerOperation::SessionStart {session, request, argv, cwd, env, rows, cols, placement}` (start in the holder) use the same wire kind. Which one keeps the name? Should the mobile continue start a held terminal when the Session's `WorkSource::is_managed`?
- **`transport::start_session` owner lock.** The branch serializes owner starts per Session (`primary.rs` background start, `session::started`). Main sends that path to `start_held`. Does the lock still earn its place for bare launches?
- **Owner loop moved.** Main moved `transport::serve` into `transport/hosted.rs`; the branch's `Event::ProgramStatus` relay has to be ported there. Do held terminals (`transport/registry.rs`) also need to relay `ProgramStatus` to phones?
- **service.rs.** The branch split connection handling (`ticket.rs`, `remote.rs`, bulk tickets, `connection(...)`); main added `terminals: Option<Registry>`, `Host` tickets and attach-by-terminal to the old `Connection` struct. Should the remote (Tailscale WebSocket) carrier bridge held terminals?

Conflicted paths: `ops/human_session/primary.rs`, `server/{README.md,mod.rs,service.rs,subscription_measurement.rs,wire.rs}`, `transport.rs`, `tests/fixtures/dto/server/session_start.json`, `tests/transport_tests.rs`, `swift/Loopflow/Services/ServerProtocol.swift`, `swift/LoopflowTests/{ServerClientTests,ServerProtocolTests}.swift`, `tests/fixtures/dto/server_schema.json`.

### Answers (Jack Heart, 2026-11-12)

- Revision: "Keep 5" — the merged protocol stays REVISION/MIN_REVISION 5.
- `session.start`: "Phone uses held start" — main keeps `session.start`; the phone's Continue uses held-terminal start for managed Sessions; the branch's operation, its per-Session owner lock and `--start-only` go.
- Phone → held terminals: "Yes, bridge them" — the Tailscale WebSocket carrier attaches to held terminals and relays `ProgramStatus`.
