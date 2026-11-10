# LOO-801 shipping contract

The three one-shot screen reads are implemented. Desktop's outline keeps
`plan.list`. Each read has a complete Rust/Swift DTO, CLI/API parity and an
`x-result` schema. Board rows retain the existing Task state and section;
explicit Wave selection filters rows. Task detail resolves stable IDs outside
the current checkout and reads the recorded primary Session and PR checks.

`waves::show_json` is removed; serialization happens at the output boundary.
The richer Desktop reads still have consumers. Task detail reuses the resolved
Task through `registered_task_status`.

Remaining: CI repair as needed, merge and completion. Ready PR: #1869.
Review and affected checks passed. No product decision is pending. `scratch/questions.md` records
reversible schema choices. The delivery sync on 2026-11-09 merged pinned main `7cf2d25b3`; conflicts in
server docs, read dispatch and the schema fixture retain both sets of operations.
The two nested Session fixtures include main's nullable transition field.

## Concept review — 2026-11-09

Outcome: no design change and no open product choice; advance. The interaction
is board → selected Task → recorded primary Session, with complete typed reads
at each step. These projections reuse Task grouping, process observations and
terminal occupancy; they add no persisted screen state or execution lifecycle.
The existing usage docs describe this interaction and its recovery adequately.

Counterexamples considered: unavailable planning must not become an empty board;
a lost connection must not imply a dead Session; a newer Graph run must not
replace the primary conversation; an old check observation must not imply that
the latest PR head passed. The contract retains unavailable/unknown states,
primary identity, and check head/time. Retry reads after recovery without
starting execution. Keep the recorded-terminal meaning of `control_holder`
from `scratch/questions.md`; it does not authorize input.

Next show this path in QA, including an unavailable host and retry. QA measured a 256-Task existing-store snapshot; results are below. Walkthrough: `scratch/concept-review.html`.

Gate reaches `lf::commands::{screens,read,waves,session,ssh}`, `lf::tests`,
`server::{schema,service}`, `served_reads`, `dto_fixtures`, `cli_discovery`,
`documented_commands`, `ops::task`, and binary `lf`, plus the selected
architecture, website and Swift suites.

QA on 2026-11-09: a private backup of the existing store held 256 Tasks.
`plan board --all --json` returned 64,243 bytes versus 1,643,079–1,643,096
for `plan list --all --json` (96.09% smaller), with identical Task IDs.
Three CLI wall times were 5.671/2.135/2.462 s for board and
2.590/3.108/2.509 s for plan; no reliable latency gain established.
Live CLI/server parity, Swift encoding/decoding, upgrade, missing records,
Wave filtering, passive reads, unavailable registry and server restart passed.
No production repair was needed. Full scenarios and limits: `scratch/qa.html`.

- `TMPDIR=/private/tmp LOOPFLOW_TEST_LF="$PWD/target/debug/lf" cargo test -p loopflow --test served_reads screen_reads_are_compact_passive_and_match_cli`: passed after compression; rebuilt CLI, CLI/API parity, compact payload, Wave filtering, global Task ID lookup, missing records, passive reads, primary Session and recorded checks.
- `scripts/test_desktop.sh --filter 'DTOFixtureTests/screenRead' --jobs 4 -Xswiftc -gnone`: passed; Desktop built headlessly and 3 tests passed.
- `cargo test -p loopflow --lib lf::commands::screens::tests::`: passed, 4 tests; linker warned `__eh_frame section too large (max 16MB)`.
- `cargo test -p loopflow --lib lf::tests::`: passed, 39 CLI parser/routing tests.
- `cargo fmt` and `git diff --check`: passed after compression and at publication.
- `lf open --host desktop --file rust/loopflow/src/server/README.md --file docs/dev/server.md`: write-docs reported "task_09da292684f1441d9aa138b8f7710b35 has no workspace in this Desktop window; nothing changed."

- `cargo build -p loopflow --bin lf`: passed during QA.
- `scripts/test_desktop.sh --filter DesktopHeadlessTests/workStates --jobs 4 -Xswiftc -gnone`: passed during QA; one headless production-view check. No live-window walkthrough.

Publication checks on 2026-11-09:
- `scripts/test.py --reuse-passing --rust-filter` with the affected modules listed above: architecture, inventory, formatting, CLI build and 260 Rust tests passed (`.lf/tmp/gate/run-13283`). Clippy was stopped by the host-pressure monitor after syspolicyd held 375% CPU for three samples; no code diagnostic. Website and Swift had not started.
- Retried only Clippy, website and Swift through `scripts.test.run_plans`, retaining its resource checks after syspolicyd fell to 38% CPU: all passed (`.lf/tmp/gate/run-57776`). `cargo clippy --all-targets --jobs 4 -- -D warnings`; 73 website tests; Desktop build and 785 headless Swift tests; Swift multiplatform checks. No checks deferred except optional display diagnostics.
- `target/debug/lf help --all` against the base's Clap definitions: adds only `lf plan board`, `lf task detail`, and `lf session card`.

- Sync resolution: `cargo test -p loopflow --lib server::schema::tests::the_checked_in_schema_is_what_describe_prints --jobs 4` with `LF_*` unset and closed stdin: passed, 1 test; existing linker unwind-size warning.
- After merging main `7cf2d25b3`, the same affected-suite command passed all four suites (`.lf/tmp/gate/run-62603`): 265 Rust tests, Clippy, 73 website tests, Desktop build, 797 headless Swift tests, architecture, formatting and multiplatform checks. No product failures or host-pressure interruption in this run.

Gate on published head `34425ae61`, 2026-11-09: no code or documentation repair needed. The existing PR copy matches the implementation, measurements and QA limits.

- `scripts/test.py --reuse-passing --rust-filter <affected modules above>` (`.lf/tmp/gate/run-36484`): architecture, formatting, CLI build, 73 website tests, Desktop build, 797 headless Swift tests and multiplatform checks passed. Rust selection failed before tests because this invocation spelled the integration module `binary(dto_fixtures)`; use `test(dto_fixtures::)`. Clippy did not run in this invocation.
- `scripts/test.py --base HEAD --rust --reuse-passing --rust-filter 'test(lf::commands::screens::) | test(lf::commands::read::) | test(lf::commands::waves::) | test(lf::commands::session::) | test(lf::commands::ssh::) | test(lf::tests::) | test(server::schema::) | test(server::service::) | binary(served_reads) | test(dto_fixtures::) | test(cli_discovery::) | test(documented_commands::) | test(ops::task::) | binary(lf)'` (`.lf/tmp/gate/run-21841`): 265 tests, inventory, formatting, CLI build and Clippy passed; unchanged non-Rust suites retain the preceding results. Both invocations used `uv run python`, cleared inherited `LF_*`, closed stdin, `TMPDIR=/private/tmp` and candidate CLI first on PATH. The existing linker unwind-size warning recurred; no timeout or host-pressure interruption.
- Hosted CI on this checkpoint defers its matrix while added scratch remains, as `.github/CI.md` specifies; the auto Graph's landing step clears scratch and runs candidate CI. Optional display diagnostics remain unrun.
