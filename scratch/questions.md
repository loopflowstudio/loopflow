# LOO-867 open choices

- Peer planning import reads `planning_client::connected(repo)` once for the
  imported repository and passes it to every reasserted Task; touched Tasks are
  assumed to belong to that repository.
- Rental hand-back retries the whole transaction when a new claim arrived
  after preflight: roll back, read planning connections outside the write lock,
  then recheck current ownership. No config fallback runs inside the write.
  This replaces the rare-race exception recorded at 9efef8b859.
- `apply_linear_observation` takes `planning_connected` from its callers. Both
  callers return early unless it is true, so production always passes `true`;
  the tests pass `false`, which is what the old in-store read returned for
  their unconfigured repository.
- `ingest_marker_in` has a third caller the concept review did not list:
  `task_comments::ingest_task_comment`, inside `apply_linear_observation`'s
  write. It takes the same Machine-name map, read before `BEGIN`.
- `rentals::tasks_held_in` is gone. `task_ownership::held_in` returns
  `HeldTask { task, owner, since }` from the store alone, so `tasks_held_by`
  no longer loads Machine declarations once per held Task.
- Outside Task ownership, two rental writes still load placement config after
  `BEGIN`: `set_machine_ownership` (`declared_pools`) and `admit_rental`
  (`ondemand_in`), both in `rentals.rs`. Left alone here: they are Machine
  pool/on-demand writes, not Task ownership. Moving those reads before `BEGIN`
  would be a separate Task.

## Checks

- QA at HEAD 6002856dfe (2026-11-13): rental hand-back through real `lf machine ondemand rent` / `task move --machine` / `machine end` with stand-in `uv` and `ssh`; a hand-off landed mid-end triggers the retry (3 config opens vs 2, lock free at each). `scratch/qa.html` §7.
- QA rerun at HEAD 55b8cdac69 (2026-11-13): claim walkthrough, malformed config, concurrent claims and declared-Machine owner reads pass; lock probe not rerun (no read-order change). `scratch/qa.html` §6.
- QA rerun at HEAD 3cd3700868 (2026-11-13, branch `lf`): claim/unassign walkthrough, FIFO lock probe (0 of 136 config opens under the write lock), malformed config and concurrent claims all as before. `scratch/qa.html` §5.

## Concept review (2026-11-13)

`OwnershipFacts` still holds: everything an ownership write needs from outside
the store is read before `BEGIN` and passed in as values. The merge of
87f0e3c60a touched no ownership code; no product choice is open. The remaining
in-write fallback found in that review is removed below.

`owners_in` now calls `machine_names_in` (7cde745b90), so the
declarations-to-names mapping lives once. `cargo test -p loopflow --lib
store::sqlite::task_ownership:: store::sqlite::selections::`: 29 of 29 pass.
The gate below ran at e282904291, before that commit.

## Gate rerun (2026-11-13, HEAD 9518f3911a)

- Only `task_ownership.rs` changed since e282904291 (−4 lines). `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: pass. Focused tests above cover the change; earlier suites keep their result.

## Gate (2026-11-13, HEAD e282904291, base 87f0e3c60a = origin/main)

- `scripts/test.py --reuse-passing`: stopped at resource preflight again (`~/.cache/uv` inventory deadline, 38.7 GiB free). Plan run by hand:
- `check_architecture.py`, `check_rust_test_inventory.py`, `cargo fmt --check`: pass.
- `cargo build -p loopflow --bin lf`: pass (3 min).
- nextest under `test_network.py`, filter `test(store::sqlite::{task_ownership,rentals,children::linear_observation_tests,planning_peers,task_comments}::) | test(ops::linear_observe::) | test(store::query_plans::) | test(documented_commands::) | binary(two_people)`: 36 of 36 pass (`rentals::` 9/9 in its own run, the rest 27/27).
- First attempt sat 6,700 s on four `rentals::` tests. They were not hung: `ps` showed state `T` (stopped by a signal) at 0% CPU, and `kill -CONT` let each one pass at once. Test processes in other checkouts (loo-865, loo-593) were stopped the same way; the sender is unknown and the kernel log has no swap/suspend entry. The rerun used `/tmp/loo867-cont.sh`, which resumes stopped descendants of this run only; it resumed 4 tests.
- `cargo clippy --all-targets -- -D warnings`: pass.
- `website/dev.py test` with the candidate `lf` on PATH: pass.
- CI still skips proof jobs until `scratch/` is clear at `lf pr submit`; CI's `rust-test` and `rust-lint` rerun these there.

## Docs

- Changed doc: `docs/dev/data.md` (write-transaction rules: config loads before `BEGIN`). No user-facing behavior changes, so no README or guide edits.

## Stale-preflight repair (fae6f73a1a)

`hand_back_in` returns `None` without writing when a held Task lacks planning
facts; `set_rental_state` rolls back, rereads `held_planning_in` outside the
write and retries. No `planning_connected_in` call remains after `BEGIN`
(remaining callers: `held_planning_in`, `hand_back`, `ingest_ownership_marker`,
all before the transaction). The 709e0298b6 merge after it is a ruff bump only.

- `cargo test -p loopflow --lib store::sqlite::rentals::tests::`: 10/10 pass, including `hand_back_writes_nothing_when_a_claim_lacks_planning_facts`.
- Still to run at gate for fae6f73a1a: `cargo clippy --all-targets -- -D warnings` and the rental/ownership nextest filter above. The gate results above predate it.

## Concept review (2026-11-13, HEAD 4c6a32ee3d)

Reviewed fae6f73a1a against the `OwnershipFacts` rule. It fits: `hand_back_in`
writes only from facts read before `BEGIN`, and stale facts end the attempt
instead of triggering a config read. No `planning_client::connected` or
`machine_declarations` call is reachable after `BEGIN` in a Task ownership
write; the other in-store callers (`project_planning_sync`,
`task_planning_syncs_in`, `task_comments`) are reads in savepoints. No design
change, no open product choice. The two Machine pool/on-demand writes listed
under open choices stay out of scope.

## Gate (2026-11-13, HEAD c85f6c729a)

Reviewed the full origin/main diff, including rental retry, ownership fact
construction, marker ingestion and planning/Linear callers. No code repair
needed. The retry checks all held Tasks before appending any handoff; dropping
its transaction on `continue` rolls the rental update back before preflight.

- `scripts/test.py --list`: architecture, Rust and website selected; Rust filter covers ownership, rentals, Linear observation, planning peers, comments, selections, query plans, documented commands and `two_people`.
- `uv run python scripts/test.py --reuse-passing --rust-filter <above modules>`: stopped before product checks; 25.0 GiB free below the 32.0 GiB emergency reserve, and uv-cache inventory exceeded its 15-second deadline. Recovery reclaimed nothing; this checkout has no incremental cache and only 3.5 GiB of build outputs. No compilation bypass of the reserve.
- `uv run python scripts/check_architecture.py`: pass, no unexplained owners or stale vocabulary.
- `uv run --no-sync python scripts/check_rust_test_inventory.py`: pass.
- `cargo +stable fmt --all -- --check`: pass (rustup stable 1.100.0; ambient Homebrew is 1.98.1).
- `git diff --check origin/main...HEAD -- ':!scratch'`: pass; scratch contains raw QA logs with original whitespace.
- `PATH=<checkout>/target/debug:$PATH uv run python dev.py test` in `website/`: 73/73 pass in 53.60 s; generated reference names the candidate CLI. Initial invocation used a wrong PATH and was interrupted, then corrected.
- Fresh CLI build, filtered nextest (including `documented_commands::` and `binary(two_people)`) and `cargo +stable clippy --all-targets -- -D warnings`: deferred to CI or a host above the disk reserve. Integration binaries are absent locally. Earlier passes remain historical, not coverage of the final sync and retry repair.

An overly broad process listing exposed an `LF_CAPTURE_KEY` in logged command
arguments during this run. Rotate the exposed capture credential; no value is
copied here. Later process inspection used only PID, parent, state, elapsed
time and executable name.
- `LOOPFLOW_TEST_LF=$PWD/target/debug/lf uv run --no-sync python scripts/test_network.py $PWD/target/debug/deps/loopflow-1ee21f543bd89a58 store::sqlite::task_ownership:: store::sqlite::rentals:: store::sqlite::children::linear_observation_tests:: store::sqlite::planning_peers:: store::sqlite::task_comments:: store::sqlite::selections:: ops::linear_observe:: store::query_plans:: --test-threads=2`: 54/54 pass in 113.80 s, external network denied, inherited `LF_*` cleared. Cached library binary built at 08:24:51 after the changed sources (latest 08:23:06), includes the new regression, and no Rust source changed after fae6f73a1a. This is a cached-binary run, not a fresh build or nextest result.
