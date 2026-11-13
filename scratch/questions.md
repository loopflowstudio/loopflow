# LOO-867 open choices

- Peer planning import reads `planning_client::connected(repo)` once for the
  imported repository and passes it to every reasserted Task; touched Tasks are
  assumed to belong to that repository.
- `hand_back_in` takes planning connections read before `BEGIN` for the Tasks
  the ended Machine held; a Task claimed there between that read and `BEGIN`
  loads its connection inside the write (rare race, correct result).
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

- QA rerun at HEAD 55b8cdac69 (2026-11-13): claim walkthrough, malformed config, concurrent claims and declared-Machine owner reads pass; lock probe not rerun (no read-order change). `scratch/qa.html` §6.
- QA rerun at HEAD 3cd3700868 (2026-11-13, branch `lf`): claim/unassign walkthrough, FIFO lock probe (0 of 136 config opens under the write lock), malformed config and concurrent claims all as before. `scratch/qa.html` §5.

## Concept review (2026-11-13)

`OwnershipFacts` still holds: everything an ownership write needs from outside
the store is read before `BEGIN` and passed in as values. The merge of
87f0e3c60a touched no ownership code; no product choice is open. The one
remaining in-write config load is `hand_back_in`'s fallback, listed above.

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
