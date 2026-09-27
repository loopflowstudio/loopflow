# Cut D — one-time import of old Homes' Session files

2026-09-27 · LOO-298 · One bounded implementation Run on top of Cut E
(`fbb48062d`). Committed with plain git. Nothing published, installed or rebased.

## The command

```bash
lf session import --dry-run --json   # what would be stored; stores nothing
lf session import --json             # store; run once per Home
```

The plan named no command; the fixture sketch used `lf session import --from`.
It imports the Home the command runs in, selected like every other command, so
there is no `--from`.

| Source | Becomes |
| --- | --- |
| `human-sessions/<id>.json` | `ask` Session with the file's id, question, answer and completion; its Run names the caller |
| `flows/<id>/position.json` whose current step is a review | `flow_review` Session `flow:<invocation>:<boundary>` with its feedback, its Run and the invocation row |
| Run record with a terminal surface and provider history, outside any Flow | `interactive` Session whose id is the Run id; closed when `session-resolution.json` exists |
| Task review the schema draft already stored | Its name from `session-name.json` and its provider from the Run's manifest |
| `session-name.json` beside any of those Runs | The Session's title and its provenance |

A file that cannot be stored is reported with its path and reason, and the rest
proceed. Sources are never written. Each Session is one transaction keyed by
its id, so an interrupted import is finished by running it again.

## Owners now

One module, `rust/loopflow/src/ops/session_import.rs`, **538 lines**, knows the
old file shapes. It has no trait; its functions are methods of one private
struct that carries the store, the Home and the report. It can be deleted, with
`SessionCommand::Import` and the acceptance test, once every Home that ran a
release older than the Session tables has been imported.

Store changes: `create_session` writes the `ready_summary` and `completed_at`
of the Session it is given (every earlier caller passes none), and
`fill_run_provider` fills a Run's missing provider. `run_record::record_dirs`
became `pub(crate)`. No schema draft and no DTO changed.

## Line counts (lines before the test module)

| File | Before | After |
| --- | --- | --- |
| `ops/session_import.rs` | 0 | 538 |
| `lf/commands/session.rs` | 220 | 245 |
| `store/sqlite/sessions.rs` | 559 | 577 |
| `store/sessions.rs` | 140 | 155 |
| `lf/mod.rs` | 1,862 | 1,870 |
| `ops/mod.rs` | 75 | 76 |

**Net production delta: +605 lines.** Nothing was deleted.

## Commands and results

Ambient `LF_*` cleared for every cargo command; `-j 4`, `nice +10`.

| Command | Result |
| --- | --- |
| `cargo nextest run -p loopflow --test session_cutover_tests -E 'test(import_stores)'` | First: did not compile (private import path, then a double borrow in the test). Second: **1 failed**: the test expected the import to start INF-123, which its own Task review had started. The interactive record now names INF-124. Third: 1 passed |
| `cargo nextest run -p loopflow --no-fail-fast --test session_cutover_tests --test session_cli_tests --test dto_fixtures` | **24 passed, 0 failed** |
| `cargo nextest run -p loopflow --no-fail-fast --lib -E 'test(ops::human_session) \| test(ops::flow_session) \| test(ops::flow_run) \| test(run_record) \| test(store::sqlite) \| test(store::migrations) \| test(controller::task)'` | **196 passed, 0 failed** |
| `scripts/check_migrations.py` | 52 shipped migrations unchanged |
| `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` | pass, on the committed content |

One failure reason's wording changed after the two suites ran. Format, clippy
and the import test were run again on the final content; the two suites were
not.

## The installed Home, on a copy

Source, read only: `~/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea`,
schema `0.12.22.001_release`. The copy held `loopflow.db` with its WAL, six Ask
files, six Flow positions, and from 397 Run records only the manifest, name,
provider-session, resolution and client files. The branch's debug `lf` ran
against the copy with a cleared environment. Opening the copy applied the
branch's schema drafts, which stored three Task reviews.

| Run | interactive | ask | flow review | Task review | unchanged | Tasks started | not imported |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `--dry-run` | 33 | 6 | 0 | 1 | 0 | 7 | 6 |
| first import | 33 | 6 | 0 | 1 | 0 | 7 | 6 |
| second import | 0 | 0 | 0 | 0 | 40 | 0 | 6 |

After: 42 Sessions (33 interactive, 12 of them closed; 6 Asks, 3 closed; 3 Task
reviews) and 42 Runs. 10 Tasks have `started_at`, and no Task disagrees with
its Runs. `lf session list --all --json` returned 27 open Sessions with no id
twice. The dry run created no file and no Session.

All six not imported are terminal Runs of a Flow step with no stored Session:
five `review-design`, one `demo`. None of the six Flow positions waits at a
review with a Run, so no saved Flow review was imported from real data.

## Not proven

- The list was not compared with what the installed `lf` lists for that Home.
- A saved Flow review that was never opened (no Run). The fixture's review has
  a Run; the real Home had none waiting.
- A completed Ask and a closed interactive Session through the binary. The
  copy imported both; no test asserts their rows.
- An import interrupted partway. Each Session is one transaction and a second
  run stores the rest; no test kills the first.
- A Home on another schema version, or one whose Run records are large. The
  copy left out event streams.

## Decisions made here (also in `scratch/questions.md`)

Jack has reviewed none of these.

- An imported interactive Session's id is its Run id.
- A terminal Run of a Flow step that is not a stored review is not imported.
- A Run that names a Task or Wave the store does not know is not imported.
- A Task an imported Run starts gets the import's time as `started_at`.
- A completed Ask or review is closed at its file's modification time.
- An old name replaces a stored generated title, never a human one.
