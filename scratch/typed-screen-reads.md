# LOO-801 shipping contract

The three one-shot screen reads are implemented. Desktop's outline keeps
`plan.list`. Each read has a complete Rust/Swift DTO, CLI/API parity and an
`x-result` schema. Board rows retain the existing Task state and section;
explicit Wave selection filters rows. Task detail resolves stable IDs outside
the current checkout and reads the recorded primary Session and PR checks.

`waves::show_json` is removed; serialization happens at the output boundary.
The richer Desktop reads still have consumers. Task detail reuses the resolved
Task through `registered_task_status`.

Remaining: review and gate, then publication, CI repair as needed, merge
and completion. No product decision is pending. `scratch/questions.md` records
reversible schema choices. On 2026-11-09, fetched `origin/main` was
`28b0af568`, already the branch's base; delivery still needs a fresh sync.

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
- `cargo fmt` and `git diff --check`: passed after compression. Gate owns static analysis and the remaining affected suites.
- `lf open --host desktop --file rust/loopflow/src/server/README.md --file docs/dev/server.md`: write-docs reported "task_09da292684f1441d9aa138b8f7710b35 has no workspace in this Desktop window; nothing changed."

- `cargo build -p loopflow --bin lf`: passed during QA.
- `scripts/test_desktop.sh --filter DesktopHeadlessTests/workStates --jobs 4 -Xswiftc -gnone`: passed during QA; one headless production-view check. No live-window walkthrough.
