# Cut G — deletion

2026-09-27 · LOO-298 · On top of the research commit `7823b86b0`. Committed with
plain git. Nothing published, installed or rebased.

[The research](cut-g-deletion-research.md) lists every candidate. This cut
deleted the four it marked in scope and nothing else.

## Owners now

No owner changed. The saved Flow's cursor still has two owners:
`flows/<id>/position.json` drives the Flow, and the invocation row is a copy
taken before each headless step and at each review.

| Fact | Owner |
| --- | --- |
| Where a Flow step's Run keeps its record | The Run's id and the Home, through `record_dir`. The Flow's position no longer stores the path |

## Deleted (path:line at `7823b86b0`)

| Item | Where | Production lines |
| --- | --- | --- |
| `observed_run_ids`, `observed_run_ids_at` | `run_record.rs:1997–2040` | 45 |
| `RunFlowStep::boundary_key` | `run_record.rs:48,62,82` | 3 |
| `Store::position_runs` | `store/sessions.rs:9–21` | 14 |
| `Boundary::run_dir` and `bind_run`'s directory parameter | `ops/flow_run.rs:27,232,245,421` | 3, and +4 where `recover` derives the path |

A manifest or position written before this cut still parses: both fields are
ignored when present.

## Line counts (lines before the first test module)

| File | Before | After | Delta |
| --- | --- | --- | --- |
| `run_record.rs` | 2,305 | 2,255 | −50 |
| `store/sessions.rs` | 187 | 173 | −14 |
| `ops/flow_run.rs` | 464 | 465 | +1 |

**Net production delta: −63 lines.**

**Whole branch since the merge base with `origin/main` (`4cd64be3d`): +1,376.**
It was +1,331 before Cut F and +1,439 after it. Cuts F and G together added 45.

The branch is still larger than main. The deletion that would change that is
the position file, about −330 against +150, and the research names what blocks
it.

## Could not delete

Every item is in the research with its callers and blocker:
`position.json` and `flow_run::{read,write,update}`; the `prepared` marker;
`resolve_manifest`; string subjects in manifests; `SessionTitleSource`;
`WorkCatalog`; `SqliteStore::position_runs`; `terminal.json` as the state Flow
recovery reads; the two `historical_*` columns; `human-sessions/` lock files;
the import module; the Started event.

## Commands and results

Ambient `LF_*` cleared for every cargo command; `-j 4`, `nice +10`. Load 27–50.

| Command | Result |
| --- | --- |
| `cargo check -p loopflow --all-targets` | clean |
| `cargo fmt --all --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass |
| `cargo nextest run -p loopflow --no-fail-fast --test session_cutover_tests --test session_cli_tests --test dto_fixtures --test flow_tests` | **41 passed, 0 failed** |
| `cargo nextest run -p loopflow --no-fail-fast --lib -E 'test(ops::) \| test(run_record) \| test(store::) \| test(controller::task) \| test(lf::commands)'` | **617 passed, 1 failed**: `ops::pm::oauth_tests::pm_read_linear_oauth_sqlite_contention_has_bounded_failure_and_recovers`, "credential generation changed" |
| That test alone, three times | passed, passed, **failed** |

The failing test races two writers for a Linear token under SQLite contention.
This cut changed nothing it runs. It passed in both of Cut F's runs of the same
command. Whether it fails at earlier commits was not established.

`scripts/check_migrations.py` was not run again: this cut touched no SQL.

## Tests changed

- `run_record::tests::two_runs_about_one_task_keep_distinct_identity_and_shared_provenance`
  read its two Runs back through the deleted scan. It now reads each Run's
  manifest.
- A test in `lf/commands/run.rs` lost one assertion, that the scan found two
  Runs naming `task:LOO-267`. The rest of the test is unchanged.
- Six `bind_run` calls in `ops/flow_run.rs` tests lost the directory argument,
  and one assertion no longer names `run_dir`.

## Not proven

- `flow_run::recover` for a Flow whose step ran under a different Home than
  the one recovering it. The path was stored; it is now derived from the
  recovering process's Home.
- A reserved Run published before this cut and reconciled after it.
  `reconcile_reserved_manifest` compares the whole manifest, and the older one
  carries `boundary_key`.
- Swift was read, not built or tested. No Swift changed.

## Decisions made here (also in `scratch/questions.md`)

Jack has reviewed none of these.

- The position file stays the cursor owner in this cut.
- A Flow step's record path is derived from the recovering Home.
- The flaky OAuth contention test was left as it is.
