# LOO-806 — typed served writes

The Task brief calls for each served write to return its declared Rust type,
with `x-result` generated from that type, and every engine child-command
spelling checked by Clap. Existing payloads and request ownership stay unchanged.

The operation declaration now owns its result type and handler. The generated
match checks that return type, retains it in an untagged result enum, and the
server serializes the answer. Session binding, Wave sync and Desktop writes
use typed shared results. No persistence migration or product decision remains.

Delete — do not maintain (removed in this diff):
- Independent write `x-result` annotations and untyped `value` dispatch.
- JSON round trips in served Session binding, Wave sync and Desktop writes.
- Inline engine child argument builders outside parser coverage; production
  and the parser test now use the same spelling functions.
- `Write::routed`: Session preparation parses once and chooses local or remote
  execution there, keeping remote failures from falling through to local work.
- `desktop::saved_intent`: `save_arrangement` dispatches directly to typed
  Dashboard, layout and tile writers; no second arrangement dispatcher.

The PR landing spelling builds one argument vector. Parser coverage runs
fixed Task spellings once, outside the optional-argument cases.

Remaining: auto Graph review, QA, sync and delivery. Gate covers `server::schema`,
`server::service`, `lf::commands::write`, `lf::commands::desktop`,
`lf::commands::session`, `lf::tests`, affected `engine` modules, `binary(lf)` and `served_reads`
(CLI/server parity, binding and request attribution), plus formatting and clippy.
Keep other Tasks' changes when syncing before delivery.

- `cargo run -p loopflow --bin lf -- server describe --json`: generated schema
  is byte-identical to `tests/fixtures/dto/server_schema.json`.
- `cargo test -p loopflow --lib -- lf::commands::write::tests:: engine::tests::every_child_spelling_parses server::schema::tests::the_checked_in_schema_is_what_describe_prints` (inherited `LF_*` unset, stdin closed): 6 passed after compression. Apple linker warned that `__eh_frame` exceeds its compact-unwind limit; no Rust code warnings.
- `cargo test -p loopflow --test served_reads server_operations_match_their_lf_commands` (inherited `LF_*` unset, stdin closed): passed after compression, 106.62 seconds.
- `cargo fmt` and `git diff --check`: passed; clippy and affected suites remain with gate.
- Docs presentation from the prior step: `lf open --host desktop --file rust/loopflow/src/server/README.md --file docs/dev/server.md` failed with "has no workspace in this Desktop window; nothing changed." The docs remain in place.
