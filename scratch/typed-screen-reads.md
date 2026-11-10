# LOO-801 shipping contract

The three one-shot screen reads are implemented. Desktop's outline keeps
`plan.list`. Each read has a complete Rust/Swift DTO, CLI/API parity and an
`x-result` schema. Board rows retain the existing Task state and section;
explicit Wave selection filters rows. Task detail resolves stable IDs outside
the current checkout and reads the recorded primary Session and PR checks.

Delete — do not maintain: `waves::show_json`; serialize `show_snapshot` at
the output boundary. No remaining deletion targets: the richer Desktop reads
still have consumers. Task detail feeds its resolved Task to
`registered_task_status`, avoiding a repository lookup and second Task lookup.

Remaining: Graph review, QA and gate; sync main before delivery, then publish,
CI repair, merge and completion in the caller's auto Graph. No product decision
is pending. `scratch/questions.md` records reversible schema choices.

Gate reaches `lf::commands::{screens,read,waves,session,ssh}`, `lf::tests`,
`server::{schema,service}`, `served_reads`, `dto_fixtures`, `cli_discovery`,
`documented_commands`, `ops::task`, and binary `lf`, plus the selected architecture, website
and Swift suites. Use `TMPDIR=/private/tmp` for served_reads and pin the prepared
CLI with `LOOPFLOW_TEST_LF` to avoid a fixture Cargo build.

- `TMPDIR=/private/tmp LOOPFLOW_TEST_LF="$PWD/target/debug/lf" cargo test -p loopflow --test served_reads screen_reads_are_compact_passive_and_match_cli`: passed after compression; rebuilt CLI, CLI/API parity, compact payload, Wave filtering, global Task ID lookup, missing records, passive reads, primary Session and recorded checks.
- `scripts/test_desktop.sh --filter 'DTOFixtureTests/screenRead' --jobs 4 -Xswiftc -gnone`: passed; Desktop built headlessly and 3 tests passed.
- `cargo test -p loopflow --lib lf::commands::screens::tests::`: passed, 4 tests; linker warned `__eh_frame section too large (max 16MB)`.
- `cargo test -p loopflow --lib lf::tests::`: passed, 39 CLI parser/routing tests.
- `cargo fmt` and `git diff --check`: passed after compression. Gate owns static analysis and the remaining affected suites.
- `lf open --host desktop --file rust/loopflow/src/server/README.md --file docs/dev/server.md`: write-docs reported "task_09da292684f1441d9aa138b8f7710b35 has no workspace in this Desktop window; nothing changed."
