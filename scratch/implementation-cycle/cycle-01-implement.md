# Cycle 1 — implement: shared Session naming

## Result

Every `SessionRecord` now carries one canonical `title` plus required
`title_source` (`generated` | `human`). The name lives with the Session's Run,
in `<run dir>/session-name.json`, so Interactive, Ask and Flow Sessions share
one owner. Absent a stored name, readers derive the seed without writing:

| Kind | Seed |
| --- | --- |
| Interactive | invoked skill (`manifest.skill`), else stable `magical-musical` pair from the Run ID |
| Flow | current step's skill (was the Task title) |
| Ask | existing bounded question title (see questions) |

`lf session rename <id> <name…> [--suggest] [--json]` writes through
`human_session::rename` and returns the authoritative record. Without
`--suggest` the name is human; `--suggest` is a generated suggestion that
leaves a human name in place (exit 0, "keeps its human-assigned name").
Read-modify-write runs under an fs2 lock on `.session-name.lock`, then an
atomic staging rename, so a racing suggestion can never overwrite a human
name. Names are trimmed, one line, ≤80 chars; failures leave the old name.

Recovered generator: `git show 309575f8e` (collapse runtime, 2026-07-13)
deleted `engine/naming.rs`'s `MAGICAL`/`MUSICAL` lists and random
`magical-musical` pair. Restored the lists verbatim as `naming::word_pair(seed)`,
deterministic via FNV-1a so listing needs no writes. History search found
no animal word list — only `swift-falcon` test fixtures — so "animal" is not
invented. Removed the context-scraping `session_title`/`concise_title` path.

Guidance: builtin `LOOPFLOW.md` (Speak) now tells agents to improve a
generated name with `lf session rename "$LF_RUN_ID" "<name>" --suggest`,
keep human names, and never rename Task/worktree/branch. Goldens regenerated
(5 files, only that paragraph). README and docs/lf.md show the command.

## Changed paths

- `rust/loopflow/src/engine/naming.rs` — `word_pair`, lists, stability test
- `rust/loopflow/src/run_record.rs` — `SessionTitleSource`, `SessionName`,
  `read_session_name`, `write_session_name`, `record_dir` pub(crate), two tests
- `rust/loopflow/src/ops/human_session.rs` — `title_source` field, `session_name`
  seed resolution, `rename`, removed old title derivation; fixture test requires field
- `rust/loopflow/src/lf/mod.rs`, `lf/commands/session.rs` — `Rename` command + parse test
- `rust/loopflow/tests/session_cli_tests.rs` — two isolated CLI tests
- `rust/loopflow/src/engine/builtins/LOOPFLOW.md`, `tests/goldens/*.md` (5)
- `tests/fixtures/dto/session.json` (generated), `sessions.json` (human)
- `swift/Loopflow/Models/SessionRecord.swift` — `SessionTitleSource`, `titleSource`
- `swift/LoopflowTests/DTOFixtureTests.swift` — source assertions, missing-field rejection
- Session JSON literals gained `title_source`: GhosttyTerminalInputTests,
  WorkspaceNavigationProofTests (5), TaskMonitorProofTests (2),
  DesktopPerformanceTests, SessionsStoreTests, SessionsPolishTests,
  WorkspaceNavigationTests, LoopflowMac/SessionFixture.swift
- `README.md`, `docs/lf.md`

Existing dirty edits in `lf/mod.rs`, `docs/lf.md`, `WorkspaceNavigationTests.swift`
and elsewhere were preserved; nothing staged or committed.

## Proof

- `cargo test -p loopflow --test session_cli_tests` — 5 pass. New:
  `session_names_are_shared_and_human_names_win` (isolated Home; prepared Ask
  seeds question title; suggest → generated; human rename; later suggest kept;
  blank rejected; missing Session error; list and `open --json` readback; no
  provider clients/events written) and `raw_sessions_are_named_by_a_stable_word_pair`
  (skill-less Run with native history lists as interactive `x-y` pair, stable
  across reads, no name file written until rename).
- `cargo test -p loopflow --lib -- word_pairs human_session_names racing_suggestions
  session_action_fixtures every_session_kind` — 5 pass (includes 9-thread race:
  final name is the human one). `cli_separates_ask_completion_from_flow_decisions` passes.
- `cargo test -p loopflow --test golden_prompt` and `engine::builtins::tests` (14) pass.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all` clean; `git diff --check` clean.
- `swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter
  'DTOFixtureTests/(sessionsFixtureRoundTrips|sessionRequiresRun|flowSessionFixtureRoundTrips)|SessionsStoreTests|WorkspaceNavigationTests'`
  — 34 tests / 3 suites pass. Log `/tmp/loo291-cycle01-swift.log`.

Not run: native UI, real provider, live Home Sessions, broad gates. No real
user Session was renamed.

## Findings

- A boundary Run ID resolves through `find_session`'s interactive branch once
  provider history exists, so `$LF_RUN_ID` inside Ask/Flow Sessions renames the
  same Run the boundary reads. Before provider history, that branch bails; the
  boundary ID still works.
- Remote-Home Flow Sessions read no local name (seed only) and rename fails with
  the Run not found locally. Acceptable for this slice; route via `lf ssh`.
- Flow `title` now duplicates `detail` (both the step skill). Left `detail`
  unchanged; the native breadcrumb slice can decide whether detail should carry
  the Task title instead.
- Swift still renders `title` as before; `titleSource` is decoded but unused.

## Next slice

Native Session drill-down breadcrumb with in-place rename calling
`lf session rename --json`, optimistic-free readback into Podium's Session
reading, rejected-edit error retaining the old name, and Flow membership
projection (still absent from `SessionRecord`).
